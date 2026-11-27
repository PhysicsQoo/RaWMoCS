//! Shared MPFR production solver for full signed-complex weighted CNF.
use num_rational::BigRational;
use rug::Float;
use std::borrow::Cow;
use std::io::{BufRead, BufReader, Read};
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
// Keep the legacy default parser independent from the explicitly selected
// streaming route; differential tests guard their common record semantics.
fn parse_emitted(raw: &str, bits: u32) -> Result<Problem, String> {
    if raw.is_empty() || raw.len() > 2_000_000 {
        return Err("invalid source bytes".into());
    }
    let mut lines = raw.lines().filter(|s| !s.trim().is_empty());
    let header: Vec<_> = lines
        .next()
        .ok_or("missing header")?
        .split_whitespace()
        .collect();
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
    for line in lines {
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
    if clauses.len() != declared {
        return Err("missing clauses".into());
    }
    Ok(Problem {
        n,
        clauses,
        weights,
    })
}

const LARGE_SOURCE_BYTES: u64 = 32_000_000;
const LARGE_LINE_BYTES: u64 = 1_000_000;
const LARGE_VARIABLES: usize = 65_536;
const LARGE_CLAUSES: usize = 600_000;
const LARGE_LITERALS: usize = 4_000_000;

// Explicit experimental general route only. A bounded line reader never
// materializes the full source or accepts a partial record at a byte cap.
fn parse_large<R: BufRead>(mut reader: R, bits: u32, deadline: Instant) -> Result<Problem, String> {
    let mut consumed = 0_u64;
    let lines = std::iter::from_fn(move || loop {
        if Instant::now() >= deadline {
            return Some(Err("source deadline".into()));
        }
        let mut line = String::new();
        let cap = (LARGE_SOURCE_BYTES - consumed + 1).min(LARGE_LINE_BYTES + 1);
        let read = match reader.by_ref().take(cap).read_line(&mut line) {
            Ok(n) => n as u64,
            Err(e) => return Some(Err(format!("source read: {e}"))),
        };
        if read == 0 {
            return None;
        }
        consumed += read;
        if consumed > LARGE_SOURCE_BYTES {
            return Some(Err("source bytes cap".into()));
        }
        if read > LARGE_LINE_BYTES {
            return Some(Err("source line cap".into()));
        }
        if !line.trim().is_empty() {
            return Some(Ok(Cow::Owned(line)));
        }
    });
    parse_records(lines, bits)
}

fn parse_records<'a>(
    mut lines: impl Iterator<Item = Result<Cow<'a, str>, String>>,
    bits: u32,
) -> Result<Problem, String> {
    let header_line = lines.next().ok_or("missing header")??;
    let header: Vec<_> = header_line.split_whitespace().collect();
    if header.len() != 4 || header[0] != "p" || header[1] != "cnf" {
        return Err("bad p cnf header".into());
    }
    let n: usize = header[2].parse().map_err(|_| "bad variable count")?;
    let declared: usize = header[3].parse().map_err(|_| "bad clause count")?;
    if n > LARGE_VARIABLES || declared > LARGE_CLAUSES {
        return Err("oversize CNF".into());
    }
    let marker = lines.next().transpose()?;
    match marker.as_deref().map(str::trim) {
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
    let mut literal_items = 0_usize;
    for line in lines {
        let line = line?;
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
            literal_items = literal_items
                .checked_add(f.len() - 1)
                .ok_or("source literal cap")?;
            if literal_items > LARGE_LITERALS {
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
    if clauses.len() != declared {
        return Err("missing clauses".into());
    }
    Ok(Problem {
        n,
        clauses,
        weights,
    })
}

const MAX_SOURCE_BYTES: u64 = 2_000_000;
fn read_source(path: &std::ffi::OsStr) -> Result<String, String> {
    let mut input = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut input)
        .map_err(|e| e.to_string())?;
    if input.len() as u64 > MAX_SOURCE_BYTES {
        return Err("source bytes cap".into());
    }
    String::from_utf8(input).map_err(|e| e.to_string())
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
    solve_until(
        p,
        raw,
        bits,
        mode,
        Instant::now() + Duration::from_secs(45),
        &mut SolveDiagnostics::default(),
    )
}
fn solve_until(
    p: &Problem,
    raw: &str,
    bits: u32,
    mode: &str,
    deadline: Instant,
    diagnostics: &mut SolveDiagnostics,
) -> Result<(Outcome, &'static str), String> {
    if Instant::now() >= deadline {
        return Err("solver deadline".into());
    }
    if mode == "reference" {
        return reference(p, raw, bits).map(|a| (a, "reference"));
    }
    let separator = mode == "auto-separator";
    let frontier = mode == "auto-frontier";
    let auto = mode == "auto" || separator || frontier;
    if mode == "general-separator" || (separator && p.n == 0) {
        return general::solve_separator(p, bits, deadline).map(|a| (a, "general-separator"));
    }
    if mode == "general" || ((mode == "auto" || frontier) && p.n == 0) {
        return general::solve(p, bits, deadline).map(|a| (a, "general"));
    }
    if !auto && mode != "prejoin" {
        return Err("invalid solver".into());
    }
    let planned = if frontier {
        factor_prejoin::solve_frontier(p, raw, bits, deadline)
    } else {
        factor_prejoin::solve_until(p, raw, bits, deadline)
    };
    match planned {
        Ok(a) => Ok((a, "prejoin")),
        Err(reason)
            if auto
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
            diagnostics.fallback = Some(FallbackInfo {
                reason,
                route: if separator {
                    SolveRoute::GeneralSeparator
                } else {
                    SolveRoute::General
                },
            });
            if separator {
                general::solve_separator(p, bits, deadline).map(|a| (a, "general-separator"))
            } else {
                general::solve(p, bits, deadline).map(|a| (a, "general"))
            }
        }
        Err(reason) => Err(reason),
    }
}
fn failure(reason: &str) -> FailureKind {
    match reason {
        "general deadline" | "prejoin deadline" | "solver deadline" | "source deadline" => {
            FailureKind::Timeout
        }
        "source bytes cap"
        | "source line cap"
        | "source literal cap"
        | "oversize CNF"
        | "general frame allocation" => FailureKind::ResourceLimit,
        "unsupported input semantics" => FailureKind::UnsupportedInput,
        "source clause cap"
        | "clause width cap"
        | "initial cells cap"
        | "exact local bit cap"
        | "local budget"
        | "residual variable cap"
        | "no bounded prejoin plan"
        | "plan mask cap"
        | "prejoin planning budget" => FailureKind::RouteRejected,
        _ => FailureKind::InternalError,
    }
}
/// Supported requested decimal precision and its MPFR working bits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Precision {
    #[default]
    Digits30,
    Digits60,
    Digits100,
    Digits130,
}

impl Precision {
    pub const fn digits(self) -> u32 {
        match self {
            Self::Digits30 => 30,
            Self::Digits60 => 60,
            Self::Digits100 => 100,
            Self::Digits130 => 130,
        }
    }

    pub const fn bits(self) -> u32 {
        match self {
            Self::Digits30 => 100,
            Self::Digits60 => 200,
            Self::Digits100 => 333,
            Self::Digits130 => 432,
        }
    }

    pub const fn from_digits(digits: u32) -> Option<Self> {
        match digits {
            30 => Some(Self::Digits30),
            60 => Some(Self::Digits60),
            100 => Some(Self::Digits100),
            130 => Some(Self::Digits130),
            _ => None,
        }
    }
}

/// Requested solver mode; Frontier and Separator remain explicit opt-ins.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SolverMode {
    #[default]
    Auto,
    AutoFrontier,
    AutoSeparator,
    Prejoin,
    General,
    GeneralSeparator,
    Reference,
}

impl SolverMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::AutoFrontier => "auto-frontier",
            Self::AutoSeparator => "auto-separator",
            Self::Prejoin => "prejoin",
            Self::General => "general",
            Self::GeneralSeparator => "general-separator",
            Self::Reference => "reference",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "auto" => Some(Self::Auto),
            "auto-frontier" => Some(Self::AutoFrontier),
            "auto-separator" => Some(Self::AutoSeparator),
            "prejoin" => Some(Self::Prejoin),
            "general" => Some(Self::General),
            "general-separator" => Some(Self::GeneralSeparator),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
}

/// Actual completed route, which may differ from the requested solver mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveRoute {
    Prejoin,
    General,
    GeneralSeparator,
    Reference,
}

impl SolveRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prejoin => "prejoin",
            Self::General => "general",
            Self::GeneralSeparator => "general-separator",
            Self::Reference => "reference",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "prejoin" => Some(Self::Prejoin),
            "general" => Some(Self::General),
            "general-separator" => Some(Self::GeneralSeparator),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
}

/// Classified failure. A failure cannot contain a numerical result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    InvalidInput,
    UnsupportedInput,
    Timeout,
    ResourceLimit,
    RouteRejected,
    InternalError,
}

impl FailureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::UnsupportedInput => "UNSUPPORTED_INPUT",
            Self::Timeout => "TIMEOUT",
            Self::ResourceLimit => "RESOURCE_LIMIT",
            Self::RouteRejected => "ROUTE_REJECTED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    pub const fn exit_code(self) -> i32 {
        match self {
            Self::InvalidInput => 2,
            Self::UnsupportedInput => 3,
            Self::Timeout => 4,
            Self::ResourceLimit => 5,
            Self::RouteRejected => 6,
            Self::InternalError => 7,
        }
    }
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Requested precision selectors shared by CLI and library.
pub fn precision_bits(digits: u32) -> Option<u32> {
    Precision::from_digits(digits).map(Precision::bits)
}

/// Provisional in-process API; not a hard resource-isolated worker.
/// None preserves the existing default deadline (reference uses external timeout).
#[derive(Clone, Copy, Debug, Default)]
pub struct SolveOptions {
    pub precision: Precision,
    pub mode: SolverMode,
    pub timeout_ms: Option<u64>,
    pub large_general: bool,
}

#[derive(Clone, Copy)]
struct RawOptions<'a> {
    digits: u32,
    mode: &'a str,
    timeout_ms: Option<u64>,
    large_general: bool,
}

impl From<SolveOptions> for RawOptions<'static> {
    fn from(options: SolveOptions) -> Self {
        Self {
            digits: options.precision.digits(),
            mode: options.mode.as_str(),
            timeout_ms: options.timeout_ms,
            large_general: options.large_general,
        }
    }
}

/// Lossless representation of a finite *stored MPFR value*, not an exact
/// mathematical count. The value is significand times two to exponent2.
/// Signed zero is retained separately; zero has canonical exponent zero.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MpfrBinary {
    pub significand: String,
    pub exponent2: i32,
    pub precision_bits: u32,
    pub negative_zero: bool,
}

/// Owns the already-computed MPFR value; no decimal reparse or f64 conversion.
#[derive(Debug)]
pub struct MpfrValue(Float);

impl MpfrValue {
    pub fn binary(&self) -> MpfrBinary {
        let (integer, exponent2) = self.0.to_integer_exp().expect("finite result invariant");
        MpfrBinary {
            significand: integer.to_string(),
            exponent2: if self.0.is_zero() { 0 } else { exponent2 },
            precision_bits: self.0.prec(),
            negative_zero: self.0.is_zero() && self.0.is_sign_negative(),
        }
    }
}

/// A recognized router rejection that triggered General fallback.
/// The route is attempted, not a guarantee of completion.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FallbackInfo {
    pub reason: String,
    pub route: SolveRoute,
}

/// Source statistics for the explicitly admitted streaming route.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceAdmission {
    pub variables: usize,
    pub clauses: usize,
    pub literals: usize,
    pub parse_ns: u128,
}

/// Call-local diagnostics, also retained on failure. Kernel-specific stderr
/// metrics remain unchanged until the next diagnostic migration.
#[derive(Debug, Default)]
pub struct SolveDiagnostics {
    pub fallback: Option<FallbackInfo>,
    pub streaming_source: Option<SourceAdmission>,
}

/// Complete, rounded MPFR result. Boolean satisfiability is independent of count.
/// Decimal strings use the existing CLI transport precision, not a certified interval.
/// The original finite values are retained for lossless binary export on demand.
#[derive(Debug)]
pub struct SolveReport {
    pub sat: bool,
    pub real: String,
    pub imag: String,
    pub route: SolveRoute,
    pub precision: Precision,
    pub solve_ns: u128,
    real_mpfr: MpfrValue,
    imag_mpfr: MpfrValue,
    pub diagnostics: SolveDiagnostics,
}

impl SolveReport {
    pub fn real_mpfr(&self) -> &MpfrValue {
        &self.real_mpfr
    }

    pub fn imag_mpfr(&self) -> &MpfrValue {
        &self.imag_mpfr
    }
}

/// A failed solve carries no numerical count.
#[derive(Debug)]
pub struct SolveFailure {
    pub kind: FailureKind,
    reason: String,
    pub diagnostics: SolveDiagnostics,
}

impl SolveFailure {
    #[doc(hidden)]
    pub fn from_reason(reason: String) -> Self {
        let kind = if reason.starts_with("INPUT:") {
            FailureKind::InvalidInput
        } else {
            failure(&reason)
        };
        Self {
            kind,
            reason,
            diagnostics: SolveDiagnostics::default(),
        }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn into_reason(self) -> String {
        self.reason
    }

    pub const fn exit_code(&self) -> i32 {
        self.kind.exit_code()
    }
}

impl std::fmt::Display for SolveFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.reason)
    }
}

impl std::error::Error for SolveFailure {}

#[derive(Clone, Copy)]
enum Source<'a> {
    File(&'a std::ffi::OsStr),
    Text(&'a str),
}

/// Solve an emitted weighted CNF file using the existing bounded source admission.
pub fn solve_file(
    path: &std::ffi::OsStr,
    options: SolveOptions,
) -> Result<SolveReport, SolveFailure> {
    solve_source(Source::File(path), options.into())
}

/// Solve an emitted weighted CNF already held by the caller.
pub fn solve_text(raw: &str, options: SolveOptions) -> Result<SolveReport, SolveFailure> {
    solve_source(Source::Text(raw), options.into())
}

/// CLI-only compatibility boundary: preserves legacy validation/error precedence.
#[doc(hidden)]
pub fn solve_file_cli(
    path: &std::ffi::OsStr,
    digits: u32,
    mode: &str,
    timeout_ms: Option<u64>,
    large_general: bool,
) -> Result<SolveReport, SolveFailure> {
    solve_source(
        Source::File(path),
        RawOptions {
            digits,
            mode,
            timeout_ms,
            large_general,
        },
    )
}

fn solve_source(
    source: Source<'_>,
    options: RawOptions<'_>,
) -> Result<SolveReport, SolveFailure> {
    let mut diagnostics = SolveDiagnostics::default();
    match solve_source_inner(source, options, &mut diagnostics) {
        Ok(mut report) => {
            report.diagnostics = diagnostics;
            Ok(report)
        }
        Err(reason) => {
            let mut error = SolveFailure::from_reason(reason);
            error.diagnostics = diagnostics;
            Err(error)
        }
    }
}

fn solve_source_inner(
    source: Source<'_>,
    options: RawOptions<'_>,
    diagnostics: &mut SolveDiagnostics,
) -> Result<SolveReport, String> {
    let RawOptions {
        digits,
        mode,
        timeout_ms,
        large_general,
    } = options;
    let precision = Precision::from_digits(digits).ok_or("INPUT: unsupported digits")?;
    let bits = precision.bits();
    if timeout_ms.is_some_and(|ms| ms > 86_400_000) {
        return Err("INPUT: invalid timeout-ms (expected 0..86400000)".into());
    }
    if mode == "reference" && timeout_ms.is_some() {
        return Err("INPUT: reference uses external timeout only".into());
    }
    if large_general && mode != "general" {
        return Err("INPUT: --large-general requires --solver general".into());
    }
    let parse_start = Instant::now();
    let large_deadline = parse_start + Duration::from_millis(timeout_ms.unwrap_or(45_000));
    let raw = if large_general {
        Cow::Borrowed("")
    } else {
        match source {
            Source::File(path) => Cow::Owned(read_source(path).map_err(|e| {
                if e == "source bytes cap" {
                    e
                } else {
                    format!("INPUT: {e}")
                }
            })?),
            Source::Text(text) => {
                if text.len() as u64 > MAX_SOURCE_BYTES {
                    return Err("source bytes cap".into());
                }
                Cow::Borrowed(text)
            }
        }
    };
    let parsed = if large_general {
        match source {
            Source::File(path) => {
                let reader =
                    BufReader::new(std::fs::File::open(path).map_err(|e| format!("INPUT: {e}"))?);
                parse_large(reader, bits, large_deadline)
            }
            Source::Text(text) => parse_large(std::io::Cursor::new(text), bits, large_deadline),
        }
    } else {
        parse_emitted(&raw, bits)
    };
    let problem = parsed.map_err(|e| {
        if matches!(
            e.as_str(),
            "oversize CNF"
                | "unsupported input semantics"
                | "source bytes cap"
                | "source line cap"
                | "source literal cap"
                | "source deadline"
        ) {
            e
        } else {
            format!("INPUT: {e}")
        }
    })?;
    if !matches!(
        mode,
        "auto"
            | "auto-frontier"
            | "auto-separator"
            | "prejoin"
            | "general"
            | "general-separator"
            | "reference"
    ) {
        return Err("INPUT: invalid solver".into());
    }
    if large_general {
        let admission = SourceAdmission {
            variables: problem.n,
            clauses: problem.clauses.len(),
            literals: problem.clauses.iter().map(Vec::len).sum::<usize>(),
            parse_ns: parse_start.elapsed().as_nanos(),
        };
        eprintln!(
            "RAWMOCS_SOURCE_ADMITTED variables={} clauses={} literals={} parse_ns={}",
            admission.variables, admission.clauses, admission.literals, admission.parse_ns
        );
        diagnostics.streaming_source = Some(admission);
    }
    let start = Instant::now();
    let deadline = if large_general {
        large_deadline
    } else {
        start + Duration::from_millis(timeout_ms.unwrap_or(45_000))
    };
    let (answer, route) = if large_general {
        (general::solve_large(problem, bits, deadline)?, "general")
    } else {
        solve_until(&problem, &raw, bits, mode, deadline, diagnostics)?
    };
    if mode != "reference" && Instant::now() >= deadline {
        return Err("solver deadline".into());
    }
    if !answer.count.re.is_finite() || !answer.count.im.is_finite() {
        return Err("nonfinite result".into());
    }
    // Format the complete result before exposing any SAT/count line.
    let real = answer
        .count
        .re
        .to_string_radix(10, Some(digits as usize + 4));
    let imag = answer
        .count
        .im
        .to_string_radix(10, Some(digits as usize + 4));
    Ok(SolveReport {
        sat: answer.sat,
        real,
        imag,
        route: SolveRoute::from_name(route).ok_or("invalid internal route")?,
        precision,
        solve_ns: start.elapsed().as_nanos(),
        real_mpfr: MpfrValue(answer.count.re),
        imag_mpfr: MpfrValue(answer.count.im),
        diagnostics: SolveDiagnostics::default(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_mpfr_binary_is_lossless() {
        for bits in [100, 200, 333, 432] {
            for text in ["0", "-0", "0.1", "-0.125", "1e1000", "-1e-1000"] {
                let original = Float::with_val(bits, Float::parse(text).unwrap());
                let binary = MpfrValue(original.clone()).binary();
                let integer = rug::Integer::from_str_radix(&binary.significand, 10).unwrap();
                let mut restored = Float::with_val(binary.precision_bits, integer);
                restored <<= binary.exponent2;
                if binary.negative_zero {
                    restored = Float::with_val(bits, rug::float::Special::NegZero);
                }
                assert_eq!(restored, original);
                assert_eq!(restored.is_sign_negative(), original.is_sign_negative());
                assert_eq!(binary.precision_bits, bits);
            }
        }
    }

    #[test]
    fn streamed_general_matches_small_parser() {
        for bits in [100, 200, 333, 432] {
            for raw in [
                "p cnf 4 2\nc t wmc\nc p weight 1 -0.5 0.25 0\n1 2 0\n-1 3 0\n",
                "p cnf 1 0\r\nc t wmc\r\nc p weight 1 -1 0 0",
                "p cnf 0 1\nc t wmc\n0\n",
                "p cnf 4 2\nc t wmc\n1 2 0\n3 4 0\n",
            ] {
                let deadline = Instant::now() + Duration::from_secs(30);
                let p = parse_emitted(raw, bits).unwrap();
                let q = parse_large(std::io::Cursor::new(raw), bits, deadline).unwrap();
                assert_eq!(p.n, q.n);
                assert_eq!(p.clauses, q.clauses);
                let a = general::solve(&p, bits, deadline).unwrap();
                let b = general::solve(&q, bits, deadline).unwrap();
                assert_eq!(a.sat, b.sat);
                assert_eq!(a.count.re, b.count.re);
                assert_eq!(a.count.im, b.count.im);
            }
        }
    }

    #[test]
    fn independent_parsers_preserve_shared_record_semantics() {
        let deadline = Instant::now() + Duration::from_secs(30);
        for bits in [100, 200, 333, 432] {
            for raw in [
                "p cnf 2 2\nc t wmc\nc p weight 1 -0.125 0.25 0\nc p weight -1 2e-3 -3e-4 0\n1 1 -2 0\n-1 2 0\n",
                "\n p cnf 2 1\r\n\r\n c t wmc \r\n c p weight -2 0 0 0\r\n1 -1 0",
                "p cnf 0 0\nc t wmc\n",
                "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",
                "p cnf 1 1\nc t wmc\n0\n",
                "p cnf 1 1\nc t wmc\n2 0\n",
                "p cnf 1 1\nc t wmc\n1\n",
                "p cnf 1 1\nc t wmc\n1 0 0\n",
                "p cnf 1 1\nc t wmc\n",
                "p cnf 1 0\nc t wmc\n1 0\n",
                "p cnf 1 1\nc t wmc\n1 0\nc p weight 1 1 0 0\n",
                "p cnf 1 0\nc t wmc\nc p weight 1 1 0 0\nc p weight 1 2 0 0\n",
                "p cnf 1 0\nc t wmc\nc p weight 0 1 0 0\n",
                "p cnf 1 0\nc t wmc\nc p weight 1 NaN 0 0\n",
                "p cnf 1 0\nc t wmc\nc p weight 1 1/2 0 0\n",
                "p cnf 1 0\nc t pwmc\n",
                "p cnf 1 0\n",
                "p dnf 1 0\nc t wmc\n",
            ] {
                let a = parse_emitted(raw, bits);
                let b = parse_large(std::io::Cursor::new(raw), bits, deadline);
                match (a, b) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(a.n, b.n);
                        assert_eq!(a.clauses, b.clauses);
                        for (x, y) in a.weights.iter().zip(&b.weights) {
                            assert_eq!(x.positive.re, y.positive.re);
                            assert_eq!(x.positive.im, y.positive.im);
                            assert_eq!(x.negative.re, y.negative.re);
                            assert_eq!(x.negative.im, y.negative.im);
                        }
                    }
                    (Err(a), Err(b)) => assert_eq!(a, b, "{raw:?}"),
                    _ => panic!("parser admission mismatch: {raw:?}"),
                }
            }
        }
    }

    #[test]
    fn streamed_limits_and_invalid_records_fail_closed() {
        let deadline = Instant::now() + Duration::from_secs(30);
        for raw in [
            "p cnf 65537 0\nc t wmc\n",
            "p cnf 1 600001\nc t wmc\n",
            "p cnf 1 1\nc t wmc\n2 0\n",
            "p cnf 1 1\nc t wmc\n1\n",
            "p cnf 1 0\nc t pwmc\n",
            "p cnf 1 0\nc t wmc\nc p weight 1 1 0 0\nc p weight 1 1 0 0\n",
        ] {
            assert!(parse_large(std::io::Cursor::new(raw), 100, deadline).is_err());
        }
        let long_line = " ".repeat(LARGE_LINE_BYTES as usize + 1);
        assert_eq!(
            parse_large(std::io::Cursor::new(long_line), 100, deadline)
                .err()
                .as_deref(),
            Some("source line cap")
        );
        assert_eq!(
            parse_large(
                std::io::Cursor::new("p cnf 0 0\nc t wmc\n"),
                100,
                Instant::now()
            )
            .err()
            .as_deref(),
            Some("source deadline")
        );
        let line = " ".repeat(999_999) + "\n";
        let over_bytes = line.repeat(33);
        assert_eq!(
            parse_large(std::io::Cursor::new(over_bytes), 100, deadline)
                .err()
                .as_deref(),
            Some("source bytes cap")
        );
        let row = "1 ".repeat(100_000) + "0\n";
        let over_literals = "p cnf 1 41\nc t wmc\n".to_owned() + &row.repeat(41);
        assert_eq!(
            parse_large(std::io::Cursor::new(over_literals), 100, deadline)
                .err()
                .as_deref(),
            Some("source literal cap")
        );
        let raw = "p cnf 4097 0\nc t wmc\n";
        assert!(parse_emitted(raw, 100).is_err());
        assert_eq!(
            parse_large(std::io::Cursor::new(raw), 100, deadline)
                .unwrap()
                .n,
            4097
        );
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
    fn auto_frontier_preserves_existing_routes_on_small_cases() {
        for raw in [
            "p cnf 2 1\nc t wmc\n1 2 0\n",
            "p cnf 10 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n",
            "p cnf 0 0\nc t wmc\n",
            "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",
            "p cnf 1 2\nc t wmc\n1 0\n-1 0\n",
        ] {
            for bits in [100, 200, 333, 432] {
                let p = parse_emitted(raw, bits).unwrap();
                let (a, route) = solve(&p, raw, bits, "auto-frontier").unwrap();
                let (b, old_route) = solve(&p, raw, bits, "auto").unwrap();
                assert_eq!(route, old_route);
                assert_eq!(a.sat, b.sat);
                assert_eq!(a.count.re, b.count.re);
                assert_eq!(a.count.im, b.count.im);
            }
        }
    }

    #[test]
    fn auto_separator_preserves_prejoin_and_fallback_contract() {
        for (raw, expected_route) in [
            ("p cnf 2 1\nc t wmc\n1 2 0\n", "prejoin"),
            (
                "p cnf 10 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n",
                "general-separator",
            ),
            ("p cnf 0 0\nc t wmc\n", "general-separator"),
        ] {
            for bits in [100, 200, 333, 432] {
                let p = parse_emitted(raw, bits).unwrap();
                let (a, route) = solve(&p, raw, bits, "auto-separator").unwrap();
                let (b, _) = solve(&p, raw, bits, "auto").unwrap();
                assert_eq!(route, expected_route);
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
