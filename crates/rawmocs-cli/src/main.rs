//! First release candidate: bounded full signed-complex WMC CLI.
use rug::Float;
use num_rational::BigRational;
use std::io::Read;
use std::time::Instant;
mod factor_prejoin;

type Clause = Vec<i32>;

#[derive(Clone)]
struct C { re: Float, im: Float }
impl C {
    fn zero(bits: u32) -> Self {
        Self { re: Float::with_val(bits, 0), im: Float::with_val(bits, 0) }
    }
    fn one(bits: u32) -> Self {
        Self { re: Float::with_val(bits, 1), im: Float::with_val(bits, 0) }
    }
    fn decimal(bits: u32, re: &str, im: &str) -> Result<Self, String> {
        let a = Float::parse(re).map_err(|e| format!("real parse: {e}"))?;
        let b = Float::parse(im).map_err(|e| format!("imag parse: {e}"))?;
        let a = Float::with_val(bits, a);
        let b = Float::with_val(bits, b);
        if !a.is_finite() || !b.is_finite() { return Err("nonfinite weight".into()); }
        Ok(Self { re: a, im: b })
    }
    #[cfg(test)]
    fn add(&self, rhs: &Self, bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, &self.re + &rhs.re),
            im: Float::with_val(bits, &self.im + &rhs.im),
        }
    }
    #[cfg(test)]
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
#[cfg_attr(not(test), allow(dead_code))]
struct Weights { positive: C, negative: C }
#[cfg_attr(not(test), allow(dead_code))]
struct Problem { n: usize, clauses: Vec<Clause>, weights: Vec<Weights> }
#[derive(Clone)]
struct Outcome { sat: bool, count: C }
impl Outcome {
    fn unsat(bits: u32) -> Self { Self { sat: false, count: C::zero(bits) } }
}
fn parse_emitted(raw: &str, bits: u32) -> Result<Problem, String> {
    if raw.is_empty() || raw.len() > 2_000_000 { return Err("invalid source bytes".into()); }
    let mut lines = raw.lines().filter(|s| !s.trim().is_empty());
    let header: Vec<_> = lines.next().ok_or("missing header")?.split_whitespace().collect();
    if header.len() != 4 || header[0] != "p" || header[1] != "cnf" {
        return Err("bad p cnf header".into());
    }
    let n: usize = header[2].parse().map_err(|_| "bad variable count")?;
    let declared: usize = header[3].parse().map_err(|_| "bad clause count")?;
    if n > 4096 || declared > 50_000 { return Err("oversize CNF".into()); }
    if lines.next().map(str::trim) != Some("c t wmc") { return Err("missing wmc marker".into()); }
    let mut weights = vec![Weights { positive: C::one(bits), negative: C::one(bits) }; n];
    let mut assigned = vec![[false; 2]; n];
    let mut clauses = Vec::with_capacity(declared);
    let mut clause_started = false;
    for line in lines {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.first() == Some(&"c") {
            if clause_started || f.len() != 7 || f[1..3] != ["p", "weight"] || f[6] != "0" {
                return Err("bad weight record".into());
            }
            let lit: i32 = f[3].parse().map_err(|_| "invalid weight literal")?;
            let var = lit.unsigned_abs() as usize;
            if lit == 0 || var > n { return Err("weight variable out of range".into()); }
            let side = usize::from(lit < 0);
            if assigned[var - 1][side] { return Err("duplicated weight".into()); }
            assigned[var - 1][side] = true;
            // Strict exact decimal grammar shared with the contraction kernel.
            factor_prejoin::decimal(f[4])?;
            factor_prejoin::decimal(f[5])?;
            let value = C::decimal(bits, f[4], f[5])?;
            if side == 0 { weights[var - 1].positive = value; }
            else { weights[var - 1].negative = value; }
        } else {
            clause_started = true;
            if f.last() != Some(&"0") { return Err("unterminated clause".into()); }
            let mut clause = Vec::with_capacity(f.len() - 1);
            for token in &f[..f.len()-1] {
                let lit: i32 = token.parse().map_err(|_| "invalid clause")?;
                if lit == 0 || lit.unsigned_abs() as usize > n {
                    return Err("invalid clause literal".into());
                }
                clause.push(lit);
            }
            clauses.push(clause);
            if clauses.len() > declared { return Err("excess clauses".into()); }
        }
    }
    if clauses.len() != declared { return Err("missing clauses".into()); }
    Ok(Problem { n, clauses, weights })
}

const MAX_SOURCE_BYTES: u64 = 2_000_000;
const HELP: &str = "rawmocs --digits {30|60|100|130} [--solver auto|prejoin|reference] FILE\n\
Full signed complex WMC for QuPRS weighted CNF. Output is rounded MPFR, not a certified interval.\n\
Exit 0: complete SAT and complex count. Exit 2: unknown/error, no count. Use an external process resource supervisor.";
fn read_source(path: &std::ffi::OsStr) -> Result<String, String> {
    let mut input = Vec::new();
    std::fs::File::open(path).map_err(|e| e.to_string())?
        .take(MAX_SOURCE_BYTES + 1).read_to_end(&mut input).map_err(|e| e.to_string())?;
    if input.len() as u64 > MAX_SOURCE_BYTES { return Err("source bytes cap".into()); }
    String::from_utf8(input).map_err(|e| e.to_string())
}
fn to_big(r: &rug::Rational) -> Result<BigRational, String> {
    r.to_string().parse::<BigRational>().map_err(|_| "rational conversion".into())
}
fn to_float(r: &BigRational, bits: u32) -> Result<Float, String> {
    let n = rug::Integer::from_str_radix(&r.numer().to_string(), 10).map_err(|_| "reference numerator conversion")?;
    let d = rug::Integer::from_str_radix(&r.denom().to_string(), 10).map_err(|_| "reference denominator conversion")?;
    Ok(Float::with_val(bits, rug::Rational::from((n, d))))
}
// Small general-CNF exact reference fallback, not an unbounded promise.
// Both literal weights are independent; never reconstruct from rounded MPFR.
fn reference(p: &Problem, raw: &str, bits: u32) -> Result<Outcome, String> {
    if p.n > 32 || p.clauses.len() > 2048 { return Err("reference size cap".into()); }
    let mut weights = vec![rawmocs_core::Weights::new(rawmocs_core::Gaussian::one(), rawmocs_core::Gaussian::one()); p.n];
    let mut weight_bits = 0u64;
    for line in raw.lines() {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.len() == 7 && f[..3] == ["c", "p", "weight"] {
            let l: i32 = f[3].parse().map_err(|_| "weight literal")?;
            let re = factor_prejoin::decimal(f[4])?;
            let im = factor_prejoin::decimal(f[5])?;
            for r in [&re, &im] {
                weight_bits += u64::from(r.numer().significant_bits()) + u64::from(r.denom().significant_bits());
            }
            if weight_bits > 4096 { return Err("reference weight bit cap".into()); }
            let w = rawmocs_core::Gaussian::new(to_big(&re)?, to_big(&im)?);
            if l > 0 { weights[l.unsigned_abs() as usize - 1].positive = w; }
            else { weights[l.unsigned_abs() as usize - 1].negative = w; }
        }
    }
    let problem = rawmocs_core::Problem::new(p.n, p.clauses.clone(), weights).map_err(|e| e.to_string())?;
    let result = rawmocs_core::solve_with_cache_payload_budget(
        &problem,
        rawmocs_core::Limits { nodes: 100_000, depth: 128, cache_entries: 10_000 },
        rawmocs_core::CachePayloadBudget { scope_items: 100_000, clause_items: 200_000, literal_items: 400_000, value_bits: 4_000_000 },
    ).map_err(|e| format!("reference: {e}"))?;
    Ok(Outcome { sat: result.satisfiable, count: C { re: to_float(&result.count.real, bits)?, im: to_float(&result.count.imag, bits)? } })
}
fn solve(p: &Problem, raw: &str, bits: u32, mode: &str) -> Result<(Outcome, &'static str), String> {
    if mode == "reference" || (mode == "auto" && p.n == 0) {
        return reference(p, raw, bits).map(|a| (a, "reference"));
    }
    if mode != "auto" && mode != "prejoin" { return Err("invalid solver".into()); }
    match factor_prejoin::solve(p, raw, bits) {
        Ok(a) => Ok((a, "prejoin")),
        // Only ordinary bounded admission failures permit fallback. A
        // replay invariant, nonfinite value or deadline never becomes SAT.
        Err(reason) if mode == "auto" && matches!(reason.as_str(),
            "source clause cap" | "clause width cap" | "initial cells cap" |
            "exact local bit cap" | "local budget" | "residual variable cap" |
            "no bounded prejoin plan" | "plan mask cap") => {
            eprintln!("RAWMOCS_FALLBACK reason={reason}");
            reference(p, raw, bits).map(|a| (a, "reference"))
                .map_err(|e| format!("prejoin: {reason}; {e}"))
        }
        Err(reason) => Err(reason),
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next().ok_or(HELP)?;
    if first == "--help" && args.next().is_none() { println!("{HELP}"); return Ok(()); }
    if first == "--version" && args.next().is_none() { println!("rawmocs {}", env!("CARGO_PKG_VERSION")); return Ok(()); }
    if first != "--digits" { return Err(HELP.into()); }
    let digits: u32 = args.next().and_then(|x| x.into_string().ok()).ok_or("missing digits")?.parse().map_err(|_| "invalid digits")?;
    let bits = match digits { 30 => 100, 60 => 200, 100 => 333, 130 => 432, _ => return Err("unsupported digits".into()) };
    let mut file = args.next().ok_or("missing source")?;
    let mut mode = "auto".to_owned();
    if file == "--solver" {
        mode = args.next().and_then(|x| x.into_string().ok()).ok_or("missing solver")?;
        file = args.next().ok_or("missing source")?;
    }
    if args.next().is_some() { return Err("unexpected arguments".into()); }
    let raw = read_source(&file)?;
    let problem = parse_emitted(&raw, bits)?;
    let start = Instant::now();
    let (answer, route) = solve(&problem, &raw, bits, &mode)?;
    if !answer.count.re.is_finite() || !answer.count.im.is_finite() { return Err("nonfinite result".into()); }
    // Format the complete result before exposing any SAT/count line.
    let sat = if answer.sat { "SATISFIABLE" } else { "UNSATISFIABLE" };
    let real = answer.count.re.to_string_radix(10, Some(digits as usize + 4));
    let imag = answer.count.im.to_string_radix(10, Some(digits as usize + 4));
    println!("s {sat}\nc r mpfr {real} {imag}");
    eprintln!("RAWMOCS_COMPLETE route={route} digits={digits} bits={bits} solve_ns={}", start.elapsed().as_nanos());
    Ok(())
}
fn main() {
    if let Err(reason) = run() {
        eprintln!("RAWMOCS_UNKNOWN: {reason}");
        println!("s UNKNOWN");
        std::process::exit(2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
    fn auto_reference_on_wide_clause_and_empty_scope() {
        let raw = "p cnf 10 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n";
        let p = parse_emitted(raw, 200).unwrap();
        let (a, route) = solve(&p, raw, 200, "auto").unwrap();
        assert_eq!(route, "reference"); assert!(a.sat); assert_eq!(a.count.re, 1023);
        for (raw, sat) in [("p cnf 0 0\nc t wmc\n", true), ("p cnf 0 1\nc t wmc\n0\n", false)] {
            let p = parse_emitted(raw, 200).unwrap();
            let (a, route) = solve(&p, raw, 200, "auto").unwrap();
            assert_eq!(route, "reference"); assert_eq!(a.sat, sat); assert_eq!(a.count.re, i32::from(sat));
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
        ] { assert!(parse_emitted(raw, 200).is_err()); }
    }
}
