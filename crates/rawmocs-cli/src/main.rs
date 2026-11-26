//! First release candidate: full signed-complex WMC CLI.
use num_rational::BigRational;
use rug::Float;
use std::io::Read;
use std::time::{Duration, Instant};
mod factor_prejoin;
mod general;

type Clause = Vec<i32>;

#[derive(Clone)]
struct C {
    re: Float,
    im: Float,
}
impl C {
    fn zero(bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, 0),
            im: Float::with_val(bits, 0),
        }
    }
    fn one(bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, 1),
            im: Float::with_val(bits, 0),
        }
    }
    fn decimal(bits: u32, re: &str, im: &str) -> Result<Self, String> {
        let a = Float::parse(re).map_err(|e| format!("real parse: {e}"))?;
        let b = Float::parse(im).map_err(|e| format!("imag parse: {e}"))?;
        let a = Float::with_val(bits, a);
        let b = Float::with_val(bits, b);
        if !a.is_finite() || !b.is_finite() {
            return Err("nonfinite weight".into());
        }
        Ok(Self { re: a, im: b })
    }
    fn add(&self, rhs: &Self, bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, &self.re + &rhs.re),
            im: Float::with_val(bits, &self.im + &rhs.im),
        }
    }
    fn mul(&self, rhs: &Self, bits: u32) -> Self {
        // GPMC ComplexMPFR does four separately rounded MPFR products.
        // rug's incomplete products can fuse, so materialize each first.
        let arbr = Float::with_val(bits, &self.re * &rhs.re);
        let aibi = Float::with_val(bits, &self.im * &rhs.im);
        let arbi = Float::with_val(bits, &self.re * &rhs.im);
        let aibr = Float::with_val(bits, &self.im * &rhs.re);
        Self {
            re: Float::with_val(bits, &arbr - &aibi),
            im: Float::with_val(bits, &arbi + &aibr),
        }
    }
}
#[derive(Clone)]
struct Weights {
    positive: C,
    negative: C,
}
struct Problem {
    n: usize,
    clauses: Vec<Clause>,
    weights: Vec<Weights>,
}
#[derive(Clone)]
struct Outcome {
    sat: bool,
    count: C,
}
impl Outcome {
    fn unsat(bits: u32) -> Self {
        Self {
            sat: false,
            count: C::zero(bits),
        }
    }
}
#[cfg(test)]
fn parse_emitted(raw: &str, bits: u32) -> Result<Problem, String> {
    parse_emitted_bounded(raw, bits, DEFAULT_SOURCE_BYTES, None)
}

fn input_deadline(deadline: Option<Instant>) -> Result<(), String> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err("input deadline".into());
    }
    Ok(())
}

fn parse_emitted_bounded(
    raw: &str,
    bits: u32,
    max_bytes: u64,
    deadline: Option<Instant>,
) -> Result<Problem, String> {
    input_deadline(deadline)?;
    if raw.len() as u64 > max_bytes {
        return Err("source bytes cap".into());
    }
    if raw.is_empty() {
        return Err("invalid source bytes".into());
    }
    let mut lines = raw.lines().filter(|s| !s.trim().is_empty());
    let header_line = lines.next().ok_or("missing header")?;
    if header_line.len() > MAX_LINE_BYTES {
        return Err("source line cap".into());
    }
    let header: Vec<_> = header_line.split_whitespace().take(5).collect();
    if header.len() != 4 || header[0] != "p" || header[1] != "cnf" {
        return Err("bad p cnf header".into());
    }
    let n: usize = header[2].parse().map_err(|_| "bad variable count")?;
    let declared: usize = header[3].parse().map_err(|_| "bad clause count")?;
    if n > 4096 || declared > 50_000 {
        return Err("oversize CNF".into());
    }
    match lines.next().map(str::trim) {
        Some("c t wmc") => {}
        Some(s) if s.starts_with("c t ") => return Err("unsupported input semantics".into()),
        _ => return Err("missing wmc marker".into()),
    }
    let mut weights = vec![
        Weights {
            positive: C::one(bits),
            negative: C::one(bits)
        };
        n
    ];
    let mut assigned = vec![[false; 2]; n];
    let mut clauses = Vec::with_capacity(declared);
    let mut clause_started = false;
    let mut literals = 0usize;
    for line in lines {
        input_deadline(deadline)?;
        if line.len() > MAX_LINE_BYTES {
            return Err("source line cap".into());
        }
        let f: Vec<_> = line.split_whitespace().collect();
        if f.first() == Some(&"c") {
            if clause_started || f.len() != 7 || f[1..3] != ["p", "weight"] || f[6] != "0" {
                return Err("bad weight record".into());
            }
            let lit: i32 = f[3].parse().map_err(|_| "invalid weight literal")?;
            let var = lit.unsigned_abs() as usize;
            if lit == 0 || var > n {
                return Err("weight variable out of range".into());
            }
            let side = usize::from(lit < 0);
            if assigned[var - 1][side] {
                return Err("duplicated weight".into());
            }
            assigned[var - 1][side] = true;
            // Strict exact decimal grammar shared with the contraction kernel.
            factor_prejoin::decimal(f[4])?;
            factor_prejoin::decimal(f[5])?;
            let value = C::decimal(bits, f[4], f[5])?;
            if side == 0 {
                weights[var - 1].positive = value;
            } else {
                weights[var - 1].negative = value;
            }
        } else {
            clause_started = true;
            if f.last() != Some(&"0") {
                return Err("unterminated clause".into());
            }
            literals += f.len() - 1;
            if literals > MAX_LITERAL_ITEMS {
                return Err("source literal cap".into());
            }
            let mut clause = Vec::with_capacity(f.len() - 1);
            for token in &f[..f.len() - 1] {
                let lit: i32 = token.parse().map_err(|_| "invalid clause")?;
                if lit == 0 || lit.unsigned_abs() as usize > n {
                    return Err("invalid clause literal".into());
                }
                clause.push(lit);
            }
            clauses.push(clause);
            if clauses.len() > declared {
                return Err("excess clauses".into());
            }
        }
    }
    input_deadline(deadline)?;
    if clauses.len() != declared {
        return Err("missing clauses".into());
    }
    Ok(Problem {
        n,
        clauses,
        weights,
    })
}

// Keep the release default; a larger source requires an explicit caller opt-in.
// These are input-admission limits, not an expanded solver or memory guarantee.
const DEFAULT_SOURCE_BYTES: u64 = 2_000_000;
const HARD_SOURCE_BYTES: u64 = 16_000_000;
const MAX_LINE_BYTES: usize = DEFAULT_SOURCE_BYTES as usize;
const MAX_LITERAL_ITEMS: usize = 2_000_000;
const HELP: &str = "rawmocs --digits {30|60|100|130} [--solver auto|prejoin|general|reference] [--timeout-ms N] [--max-source-bytes N] FILE\n\
Full signed complex WMC for QuPRS weighted CNF. Output is rounded MPFR, not a certified interval.\n\
Source bytes default 2000000; explicit 1..16000000. Other admission and solver caps remain bounded.\n\
Timeout includes input admission (except reference); external supervision is required for blocking I/O.\n\
Exit 0: complete SAT and complex count. Nonzero: classified failure, empty stdout. Use an external process resource supervisor.";
fn read_bounded_source(
    reader: impl Read,
    max_bytes: u64,
    deadline: Option<Instant>,
) -> Result<String, String> {
    let mut input = Vec::new();
    let mut reader = reader.take(max_bytes + 1);
    let mut buf = [0u8; 64 * 1024];
    loop {
        input_deadline(deadline)?;
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        };
        input.extend_from_slice(&buf[..n]);
        if input.len() as u64 > max_bytes {
            return Err("source bytes cap".into());
        }
    }
    input_deadline(deadline)?;
    String::from_utf8(input).map_err(|e| e.to_string())
}

fn read_source(
    path: &std::ffi::OsStr,
    max_bytes: u64,
    deadline: Option<Instant>,
) -> Result<String, String> {
    input_deadline(deadline)?;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    read_bounded_source(file, max_bytes, deadline)
}
fn to_big(r: &rug::Rational) -> Result<BigRational, String> {
    r.to_string()
        .parse::<BigRational>()
        .map_err(|_| "rational conversion".into())
}
fn to_float(r: &BigRational, bits: u32) -> Result<Float, String> {
    let n = rug::Integer::from_str_radix(&r.numer().to_string(), 10)
        .map_err(|_| "reference numerator conversion")?;
    let d = rug::Integer::from_str_radix(&r.denom().to_string(), 10)
        .map_err(|_| "reference denominator conversion")?;
    Ok(Float::with_val(bits, rug::Rational::from((n, d))))
}
// Small general-CNF exact reference fallback, not an unbounded promise.
// Both literal weights are independent; never reconstruct from rounded MPFR.
fn reference(p: &Problem, raw: &str, bits: u32) -> Result<Outcome, String> {
    if p.n > 32 || p.clauses.len() > 2048 {
        return Err("reference size cap".into());
    }
    let mut weights = vec![
        rawmocs_core::Weights::new(
            rawmocs_core::Gaussian::one(),
            rawmocs_core::Gaussian::one()
        );
        p.n
    ];
    let mut weight_bits = 0u64;
    for line in raw.lines() {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.len() == 7 && f[..3] == ["c", "p", "weight"] {
            let l: i32 = f[3].parse().map_err(|_| "weight literal")?;
            let re = factor_prejoin::decimal(f[4])?;
            let im = factor_prejoin::decimal(f[5])?;
            for r in [&re, &im] {
                weight_bits += u64::from(r.numer().significant_bits())
                    + u64::from(r.denom().significant_bits());
            }
            if weight_bits > 4096 {
                return Err("reference weight bit cap".into());
            }
            let w = rawmocs_core::Gaussian::new(to_big(&re)?, to_big(&im)?);
            if l > 0 {
                weights[l.unsigned_abs() as usize - 1].positive = w;
            } else {
                weights[l.unsigned_abs() as usize - 1].negative = w;
            }
        }
    }
    let problem =
        rawmocs_core::Problem::new(p.n, p.clauses.clone(), weights).map_err(|e| e.to_string())?;
    let result = rawmocs_core::solve_with_cache_payload_budget(
        &problem,
        rawmocs_core::Limits {
            nodes: 100_000,
            depth: 128,
            cache_entries: 10_000,
        },
        rawmocs_core::CachePayloadBudget {
            scope_items: 100_000,
            clause_items: 200_000,
            literal_items: 400_000,
            value_bits: 4_000_000,
        },
    )
    .map_err(|e| format!("reference: {e}"))?;
    Ok(Outcome {
        sat: result.satisfiable,
        count: C {
            re: to_float(&result.count.real, bits)?,
            im: to_float(&result.count.imag, bits)?,
        },
    })
}
#[cfg(test)]
fn solve(p: &Problem, raw: &str, bits: u32, mode: &str) -> Result<(Outcome, &'static str), String> {
    solve_until(p, raw, bits, mode, Instant::now() + Duration::from_secs(45))
}
fn solve_until(
    p: &Problem,
    raw: &str,
    bits: u32,
    mode: &str,
    deadline: Instant,
) -> Result<(Outcome, &'static str), String> {
    if Instant::now() >= deadline {
        return Err("solver deadline".into());
    }
    if mode == "reference" {
        return reference(p, raw, bits).map(|a| (a, "reference"));
    }
    if mode == "general" || (mode == "auto" && p.n == 0) {
        return general::solve(p, bits, deadline).map(|a| (a, "general"));
    }
    if mode != "auto" && mode != "prejoin" {
        return Err("invalid solver".into());
    }
    match factor_prejoin::solve_until(p, raw, bits, deadline) {
        Ok(a) => Ok((a, "prejoin")),
        Err(reason)
            if mode == "auto"
                && matches!(
                    reason.as_str(),
                    "source clause cap"
                        | "clause width cap"
                        | "initial cells cap"
                        | "exact local bit cap"
                        | "local budget"
                        | "residual variable cap"
                        | "no bounded prejoin plan"
                        | "plan mask cap"
                        | "prejoin planning budget"
                ) =>
        {
            eprintln!("RAWMOCS_FALLBACK reason={reason}");
            general::solve(p, bits, deadline).map(|a| (a, "general"))
        }
        Err(reason) => Err(reason),
    }
}
fn failure(reason: &str) -> (&'static str, i32) {
    match reason {
        "general deadline" | "prejoin deadline" | "solver deadline" | "input deadline" => {
            ("TIMEOUT", 4)
        }
        "source bytes cap"
        | "source line cap"
        | "source literal cap"
        | "oversize CNF"
        | "general frame allocation" => ("RESOURCE_LIMIT", 5),
        "unsupported input semantics" => ("UNSUPPORTED_INPUT", 3),
        "source clause cap"
        | "clause width cap"
        | "initial cells cap"
        | "exact local bit cap"
        | "local budget"
        | "residual variable cap"
        | "no bounded prejoin plan"
        | "plan mask cap"
        | "prejoin planning budget" => ("ROUTE_REJECTED", 6),
        _ => ("INTERNAL_ERROR", 7),
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next().ok_or(HELP)?;
    if first == "--help" && args.next().is_none() {
        println!("{HELP}");
        return Ok(());
    }
    if first == "--version" && args.next().is_none() {
        println!("rawmocs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if first != "--digits" {
        return Err(HELP.into());
    }
    let digits: u32 = args
        .next()
        .and_then(|x| x.into_string().ok())
        .ok_or("missing digits")?
        .parse()
        .map_err(|_| "invalid digits")?;
    let bits = match digits {
        30 => 100,
        60 => 200,
        100 => 333,
        130 => 432,
        _ => return Err("unsupported digits".into()),
    };
    let mut file = args.next().ok_or("missing source")?;
    let mut mode = "auto".to_owned();
    let mut timeout_ms = None;
    let mut source_bytes = None;
    let mut mode_seen = false;
    loop {
        if file == "--solver" && !mode_seen {
            mode = args
                .next()
                .and_then(|x| x.into_string().ok())
                .ok_or("missing solver")?;
            mode_seen = true;
        } else if file == "--timeout-ms" && timeout_ms.is_none() {
            let ms = args
                .next()
                .and_then(|x| x.into_string().ok())
                .and_then(|x| x.parse::<u64>().ok())
                .filter(|&ms| ms <= 86_400_000)
                .ok_or("INPUT: invalid timeout-ms (expected 0..86400000)")?;
            timeout_ms = Some(ms);
        } else if file == "--max-source-bytes" && source_bytes.is_none() {
            let cap = args
                .next()
                .and_then(|x| x.into_string().ok())
                .and_then(|x| x.parse::<u64>().ok())
                .filter(|&n| (1..=HARD_SOURCE_BYTES).contains(&n))
                .ok_or("INPUT: invalid max-source-bytes (expected 1..16000000)")?;
            source_bytes = Some(cap);
        } else {
            break;
        }
        file = args.next().ok_or("missing source")?;
    }
    if args.next().is_some() {
        return Err("unexpected arguments".into());
    }
    if mode == "reference" && timeout_ms.is_some() {
        return Err("INPUT: reference uses external timeout only".into());
    }
    if !matches!(mode.as_str(), "auto" | "prejoin" | "general" | "reference") {
        return Err("INPUT: invalid solver".into());
    }
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.unwrap_or(45_000));
    let parse_deadline = (mode != "reference").then_some(deadline);
    let max_bytes = source_bytes.unwrap_or(DEFAULT_SOURCE_BYTES);
    let raw = read_source(&file, max_bytes, parse_deadline).map_err(|e| {
        if matches!(e.as_str(), "source bytes cap" | "input deadline") {
            e
        } else {
            format!("INPUT: {e}")
        }
    })?;
    let problem = parse_emitted_bounded(&raw, bits, max_bytes, parse_deadline).map_err(|e| {
        if matches!(
            e.as_str(),
            "source bytes cap"
                | "source line cap"
                | "source literal cap"
                | "oversize CNF"
                | "unsupported input semantics"
                | "input deadline"
        ) {
            e
        } else {
            format!("INPUT: {e}")
        }
    })?;
    if source_bytes.is_some() {
        eprintln!(
            "RAWMOCS_SOURCE_ADMITTED bytes={} vars={} clauses={} literals={} cap={max_bytes}",
            raw.len(),
            problem.n,
            problem.clauses.len(),
            problem.clauses.iter().map(Vec::len).sum::<usize>()
        );
    }
    let start = Instant::now();
    let (answer, route) = solve_until(&problem, &raw, bits, &mode, deadline)?;
    if mode != "reference" && Instant::now() >= deadline {
        return Err("solver deadline".into());
    }
    if !answer.count.re.is_finite() || !answer.count.im.is_finite() {
        return Err("nonfinite result".into());
    }
    // Format the complete result before exposing any SAT/count line.
    let sat = if answer.sat {
        "SATISFIABLE"
    } else {
        "UNSATISFIABLE"
    };
    let real = answer
        .count
        .re
        .to_string_radix(10, Some(digits as usize + 4));
    let imag = answer
        .count
        .im
        .to_string_radix(10, Some(digits as usize + 4));
    println!("s {sat}\nc r mpfr {real} {imag}");
    eprintln!(
        "RAWMOCS_COMPLETE route={route} digits={digits} bits={bits} solve_ns={}",
        start.elapsed().as_nanos()
    );
    Ok(())
}
fn main() {
    if let Err(reason) = run() {
        let (kind, code) = if reason.starts_with("INPUT:")
            || matches!(
                reason.as_str(),
                HELP | "missing digits"
                    | "invalid digits"
                    | "unsupported digits"
                    | "missing source"
                    | "missing solver"
                    | "unexpected arguments"
            ) {
            ("INVALID_INPUT", 2)
        } else {
            failure(&reason)
        };
        eprintln!("RAWMOCS_FAILURE kind={kind}: {reason}");
        std::process::exit(code);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_source_read_and_parse_agree_at_boundary() {
        let raw = "p cnf 1 1\nc t wmc\n1 0\n";
        let size = raw.len() as u64;
        for cap in [size - 1, size, size + 1] {
            let read = read_bounded_source(raw.as_bytes(), cap, None);
            let parse = parse_emitted_bounded(raw, 100, cap, None);
            assert_eq!(read.is_ok(), cap >= size);
            assert_eq!(parse.is_ok(), cap >= size);
            if cap < size {
                assert_eq!(read.err().as_deref(), Some("source bytes cap"));
                assert_eq!(parse.err().as_deref(), Some("source bytes cap"));
            }
        }
        assert!(read_bounded_source(&[0xff][..], 1, None).is_err());
        assert!(parse_emitted_bounded("p cnf 1 1\nc t wmc\n1", 100, 100, None).is_err());
        let expired = Some(Instant::now());
        assert_eq!(
            read_bounded_source(raw.as_bytes(), size, expired).err().as_deref(),
            Some("input deadline")
        );
        assert_eq!(
            parse_emitted_bounded(raw, 100, size, expired).err().as_deref(),
            Some("input deadline")
        );
    }

    #[test]
    fn extended_source_preserves_signed_complex_and_sat_zero_semantics() {
        for (small, sat, re, im) in [
            ("p cnf 1 1\nc t wmc\nc p weight 1 -0.5 0.25 0\n1 0\n", true, -0.5, 0.25),
            ("p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n", true, 0.0, 0.0),
            ("p cnf 1 2\nc t wmc\n1 0\n-1 0\n", false, 0.0, 0.0),
        ] {
            let mut raw = small.to_owned();
            // Valid whitespace padding deliberately crosses the former byte cap.
            raw.push_str(&" \n".repeat(DEFAULT_SOURCE_BYTES as usize / 2));
            assert!(raw.len() as u64 > DEFAULT_SOURCE_BYTES);
            for bits in [100, 200, 333, 432] {
                assert_eq!(
                    parse_emitted(&raw, bits).err().as_deref(),
                    Some("source bytes cap")
                );
                let p = parse_emitted_bounded(&raw, bits, HARD_SOURCE_BYTES, None).unwrap();
                for mode in ["general", "prejoin", "reference"] {
                    let (out, _) = solve(&p, &raw, bits, mode).unwrap();
                    assert_eq!(out.sat, sat);
                    assert_eq!(out.count.re, re);
                    assert_eq!(out.count.im, im);
                }
            }
        }
    }

    #[test]
    fn exact_reference_and_prejoin_agree() {
        for bits in [100, 200, 333, 432] {
            for raw in [
                "p cnf 3 2\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight -1 0.5 0 0\n1 2 0\n-1 3 0\n",
                "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",
                "p cnf 1 2\nc t wmc\n1 0\n-1 0\n",
                "p cnf 2 0\nc t wmc\n",
            ] {
                let p = parse_emitted(raw, bits).unwrap();
                let (a, _) = solve(&p, raw, bits, "prejoin").unwrap();
                let (b, _) = solve(&p, raw, bits, "reference").unwrap();
                assert_eq!(a.sat, b.sat);
                assert_eq!(a.count.re, b.count.re);
                assert_eq!(a.count.im, b.count.im);
            }
        }
    }
    #[test]
    fn auto_general_on_wide_clause_and_empty_scope() {
        let raw = "p cnf 10 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n";
        let p = parse_emitted(raw, 200).unwrap();
        let (a, route) = solve(&p, raw, 200, "auto").unwrap();
        assert_eq!(route, "general");
        assert!(a.sat);
        assert_eq!(a.count.re, 1023);
        for (raw, sat) in [
            ("p cnf 0 0\nc t wmc\n", true),
            ("p cnf 0 1\nc t wmc\n0\n", false),
        ] {
            let p = parse_emitted(raw, 200).unwrap();
            let (a, route) = solve(&p, raw, 200, "auto").unwrap();
            assert_eq!(route, "general");
            assert_eq!(a.sat, sat);
            assert_eq!(a.count.re, i32::from(sat));
        }
    }
    #[test]
    fn general_large_route_and_exact_oracle() {
        let raw = "p cnf 33 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n";
        let p = parse_emitted(raw, 200).unwrap();
        let (a, route) = solve(&p, raw, 200, "auto").unwrap();
        assert_eq!(route, "general");
        assert!(a.sat);
        assert_eq!(a.count.re, 8_581_545_984_u64);
        assert_eq!(a.count.im, 0);
        for bits in [100, 200, 333, 432] {
            for raw in [
                "p cnf 4 2\nc t wmc\nc p weight 1 -0.5 0.25 0\nc p weight -1 0.5 0 0\n1 2 0\n-1 3 0\n",
                "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",
                "p cnf 2 2\nc t wmc\n1 0\n-1 0\n",
                "p cnf 4 2\nc t wmc\n1 2 0\n3 4 0\n",
                "p cnf 3 2\nc t wmc\n1 -1 0\n2 2 0\n",
                "p cnf 0 1\nc t wmc\n0\n",
            ] {
                let p = parse_emitted(raw, bits).unwrap();
                let (g, _) = solve(&p, raw, bits, "general").unwrap();
                let (r, _) = solve(&p, raw, bits, "reference").unwrap();
                assert_eq!(g.sat, r.sat);
                assert_eq!(g.count.re, r.count.re);
                assert_eq!(g.count.im, r.count.im);
            }
        }
        assert_eq!(
            general::solve(&p, 200, Instant::now()).err().as_deref(),
            Some("general deadline")
        );
    }
    #[test]
    fn general_large_analytic_families() {
        for bits in [100, 200, 333, 432] {
            for (raw, sat, real, imag) in [
                (
                    "p cnf 64 2\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n-11 -12 -13 -14 -15 -16 -17 -18 -19 -20 0\n",
                    true,
                    1023_i128 * 1023 * (1_i128 << 44),
                    0,
                ),
                (
                    "p cnf 40 1\nc t wmc\nc p weight 40 -1 0 0\n1 2 3 4 5 6 7 8 9 10 0\n",
                    true,
                    0,
                    0,
                ),
                (
                    "p cnf 40 1\nc t wmc\nc p weight 40 -1 0.5 0\n1 2 3 4 5 6 7 8 9 10 0\n",
                    true,
                    0,
                    1023_i128 * (1_i128 << 28),
                ),
                (
                    "p cnf 40 3\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n40 0\n-40 0\n",
                    false,
                    0,
                    0,
                ),
            ] {
                let p = parse_emitted(raw, bits).unwrap();
                let (a, route) = solve(&p, raw, bits, "auto").unwrap();
                assert_eq!(route, "general");
                assert_eq!(a.sat, sat);
                assert_eq!(a.count.re, real);
                assert_eq!(a.count.im, imag);
            }
        }
    }
    #[test]
    fn iterative_general_exact_oracle_corpus() {
        use std::fmt::Write as _;
        let mut seed = 1729_u32;
        for case in 0..64 {
            let mut raw = String::from("p cnf 6 9\nc t wmc\n");
            for v in 1..=6 {
                writeln!(raw, "c p weight {v} -0.5 0.25 0").unwrap();
                writeln!(raw, "c p weight -{v} 0.5 -0.125 0").unwrap();
            }
            for _ in 0..9 {
                for _ in 0..3 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let v = ((seed >> 16) % 6 + 1) as i32;
                    let lit = if seed & 0x100 != 0 { v } else { -v };
                    write!(raw, "{lit} ").unwrap();
                }
                raw.push_str("0\n");
            }
            for bits in [100, 200, 333, 432] {
                let p = parse_emitted(&raw, bits).unwrap();
                let (g, _) = solve(&p, &raw, bits, "general").unwrap();
                let (r, _) = solve(&p, &raw, bits, "reference").unwrap();
                assert_eq!(g.sat, r.sat, "case {case}");
                assert_eq!(g.count.re, r.count.re, "case {case}");
                assert_eq!(g.count.im, r.count.im, "case {case}");
            }
        }
    }
    #[test]
    fn strict_parser_rejects_unsupported_semantics() {
        for raw in [
            "p cnf 1 0\nc t pwmc\n",
            "p cnf 1 0\nc t wmc\nc p weight 1 NaN 0 0\n",
            "p cnf 1 0\nc t wmc\nc p weight 1 1/2 0 0\n",
            "p cnf 1 0\nc t wmc\nc p weight 1 1 0 0\nc p weight 1 2 0 0\n",
            "p cnf 1 1\nc t wmc\n2 0\n",
        ] {
            assert!(parse_emitted(raw, 200).is_err());
        }
    }
}
