//! QOO-225 research-only, fail-closed QuPRS *printed* Weighted DIMACS bridge.
//! This parses the exact rational value of finite decimal tokens.
//! It does NOT certify the quantum source phases that produced those tokens.
//!
//! Source contract: PhysicsQoo/QuPRS@0b28736651b3740ed3b201edd2efdef3da541e0d
//! test/test_wmc_direct_cnf_golden.py (blob d5d9b8ab6e513afd332e34c642a8017b7c415caa)
//! test/test_v0140_exact_printed_cnf.py (blob b4b486dd66599daca70861e1b5c8ad7db804d7f2)
//! .diagnostics/wmc_interval_reference/import_dimacs.py
//! (blob fd942b2dfe23080390bb4e151f684e2562c22d91)
//!
//! No QuPRS runtime, external solver, Python, f64, or production core changes.

use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

const MAX_VARIABLES: usize = 12;
const MAX_CLAUSES: usize = 10_000;
const MAX_SOURCE_BYTES: usize = 400_000;
const MAX_DECIMAL_BYTES: usize = 120;
const MAX_DECIMAL_EXPONENT: i32 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Gpmc,
    Ganak,
}

/// The exact *printed* finite decimal rational, including scientific notation.
/// Bounds deliberately match the QuPRS independent-reference import guard.
pub fn printed_decimal(token: &str) -> Result<BigRational, String> {
    if token.is_empty() || token.len() > MAX_DECIMAL_BYTES {
        return Err("empty or oversized decimal token".into());
    }
    let exponent_position = token.find(|c: char| c == 'e' || c == 'E');
    let (mantissa, exponent) = if let Some(index) = exponent_position {
        let exponent = token[index + 1..]
            .parse::<i32>()
            .map_err(|_| "invalid decimal exponent")?;
        if exponent.unsigned_abs() > MAX_DECIMAL_EXPONENT as u32 {
            return Err("decimal exponent exceeds reference guard".into());
        }
        (&token[..index], exponent)
    } else {
        (token, 0)
    };
    let (negative, unsigned) = if let Some(rest) = mantissa.strip_prefix('-') {
        (true, rest)
    } else {
        (false, mantissa.strip_prefix('+').unwrap_or(mantissa))
    };
    if unsigned.matches('.').count() > 1 {
        return Err("multiple decimal points".into());
    }
    let (whole, fractional) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty() && fractional.is_empty() {
        return Err("decimal token missing digits".into());
    }
    if !whole
        .bytes()
        .chain(fractional.bytes())
        .all(|c| c.is_ascii_digit())
    {
        return Err("non-decimal or non-finite token".into());
    }
    let digits = format!("{whole}{fractional}");
    let mut numerator =
        BigInt::parse_bytes(digits.as_bytes(), 10).ok_or("unable to parse decimal mantissa")?;
    if negative {
        numerator = -numerator;
    }
    let scale = fractional.len() as i32 - exponent;
    if scale >= 0 {
        let denominator = BigInt::from(10_u8).pow(scale as u32);
        Ok(BigRational::new(numerator, denominator))
    } else {
        numerator *= BigInt::from(10_u8).pow((-scale) as u32);
        Ok(BigRational::from_integer(numerator))
    }
}

fn parse_weight(fields: &[&str], dialect: Dialect) -> Result<Gaussian, String> {
    let (real, imag) = match dialect {
        Dialect::Gpmc => {
            if fields.len() != 3 || fields[2] != "0" {
                return Err("expected GPMC: <real> <imag> 0".into());
            }
            (printed_decimal(fields[0])?, printed_decimal(fields[1])?)
        }
        Dialect::Ganak => {
            if fields.len() != 4 || fields[3] != "0" || !["+", "-"].contains(&fields[1]) {
                return Err("expected Ganak: <real> +/- <imag>i 0".into());
            }
            let magnitude = fields[2]
                .strip_suffix('i')
                .ok_or("Ganak weight missing imaginary suffix")?;
            let positive_magnitude = printed_decimal(magnitude)?;
            if positive_magnitude < BigRational::from_integer(0.into()) {
                return Err("Ganak imaginary magnitude cannot be negative".into());
            }
            let imag = if fields[1] == "-" {
                -positive_magnitude
            } else {
                positive_magnitude
            };
            (printed_decimal(fields[0])?, imag)
        }
    };
    Ok(Gaussian::new(real, imag))
}

/// Accepts only the bounded QuPRS emitted weighted-CNF grammar:
/// one p cnf header, one "c t wmc", weight comments, then complete CNF rows.
/// A missing literal weight is exactly 1+0i; positive and negative weights
/// are *independent* and cannot be conflated.
///
/// No assertion about the source phase, projected counting, equivalence or
/// precision of any original floating/trigonometric coefficient is made.
pub fn parse_printed_dimacs(raw: &str, dialect: Dialect) -> Result<Problem, String> {
    if raw.is_empty() || raw.len() > MAX_SOURCE_BYTES {
        return Err("source empty or too large".into());
    }
    let mut header: Option<(usize, usize)> = None;
    let mut marker = false;
    let mut clause_started = false;
    let mut clauses: Vec<Vec<i32>> = Vec::new();
    let mut literal_weights: Vec<Option<Gaussian>> = Vec::new();

    for line in raw.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        if fields[0] == "p" {
            if header.is_some() || marker || fields.len() != 4 || fields[1] != "cnf" {
                return Err("duplicate or invalid CNF header".into());
            }
            let n = fields[2]
                .parse::<usize>()
                .map_err(|_| "invalid variable count")?;
            let count = fields[3]
                .parse::<usize>()
                .map_err(|_| "invalid clause count")?;
            if n == 0 || n > MAX_VARIABLES || count > MAX_CLAUSES {
                return Err("CNF exceeds independent reference limits".into());
            }
            header = Some((n, count));
            literal_weights = vec![None; 2 * n];
        } else if fields[0] == "c" {
            if fields.as_slice() == ["c", "t", "wmc"] {
                if header.is_none() || marker || clause_started {
                    return Err("misplaced or duplicate weighted-CNF marker".into());
                }
                marker = true;
            } else if fields.len() >= 4 && fields[..3] == ["c", "p", "weight"] {
                if !marker || clause_started {
                    return Err("weight outside source header".into());
                }
                let n = header.ok_or("weight before header")?.0;
                let lit = fields[3]
                    .parse::<i32>()
                    .map_err(|_| "invalid weighted literal")?;
                let abs = lit.unsigned_abs() as usize;
                if abs == 0 || abs > n {
                    return Err("weighted literal outside CNF scope".into());
                }
                let index = 2 * (abs - 1) + usize::from(lit < 0);
                if literal_weights[index].is_some() {
                    return Err("duplicated literal weight".into());
                }
                literal_weights[index] = Some(parse_weight(&fields[4..], dialect)?);
            } else {
                return Err("unsupported weighted DIMACS comment".into());
            }
        } else {
            if header.is_none() || !marker || fields.last() != Some(&"0") {
                return Err("clause before header or missing 0 terminator".into());
            }
            clause_started = true;
            let n = header.ok_or("missing header")?.0;
            let mut clause = Vec::new();
            for text in &fields[..fields.len() - 1] {
                let lit = text.parse::<i32>().map_err(|_| "non-numeric CNF literal")?;
                let abs = lit.unsigned_abs() as usize;
                if abs == 0 || abs > n {
                    return Err("CNF literal outside declared variables".into());
                }
                clause.push(lit);
            }
            clauses.push(clause);
            if clauses.len() > MAX_CLAUSES {
                return Err("too many clauses".into());
            }
        }
    }
    let (n, count) = header.ok_or("missing CNF header")?;
    if !marker || clauses.len() != count {
        return Err("missing weighted marker or incorrect clause count".into());
    }
    let mut weights = Vec::with_capacity(n);
    for entries in literal_weights.chunks_exact_mut(2) {
        let positive = entries[0].take().unwrap_or_else(Gaussian::one);
        let negative = entries[1].take().unwrap_or_else(Gaussian::one);
        weights.push(Weights::new(positive, negative));
    }
    Problem::new(n, clauses, weights).map_err(|e| format!("CNF rejected by core: {e}"))
}

/// Independent 2^n truth-table enumeration and schoolbook 4-real-product
/// Gaussian arithmetic. Does not invoke DPLL or Gaussian::{add,mul}.
pub fn independent_exact_full_wmc(problem: &Problem) -> Result<(bool, Gaussian), String> {
    let n = problem.variables();
    if n == 0 || n > MAX_VARIABLES {
        return Err("independent oracle size bound".into());
    }
    let mut sat = false;
    let mut real = BigRational::from_integer(0.into());
    let mut imag = BigRational::from_integer(0.into());
    for assignment in 0usize..(1usize << n) {
        if !problem.clauses().iter().all(|clause| {
            clause.iter().any(|&lit| {
                let chosen = (assignment & (1usize << (lit.unsigned_abs() as usize - 1))) != 0;
                chosen == (lit > 0)
            })
        }) {
            continue;
        }
        sat = true;
        let mut weighted_real = BigRational::from_integer(1.into());
        let mut weighted_imag = BigRational::from_integer(0.into());
        for (i, weight) in problem.weights().iter().enumerate() {
            let factor = if assignment & (1usize << i) == 0 {
                &weight.negative
            } else {
                &weight.positive
            };
            let next_real = &weighted_real * &factor.real - &weighted_imag * &factor.imag;
            let next_imag = &weighted_real * &factor.imag + &weighted_imag * &factor.real;
            weighted_real = next_real;
            weighted_imag = next_imag;
        }
        real += weighted_real;
        imag += weighted_imag;
    }
    Ok((sat, Gaussian::new(real, imag)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{solve_with_limits, Limits};

    fn rat(n: i64, d: i64) -> BigRational {
        BigRational::new(n.into(), d.into())
    }

    /// Literal weights and clauses are transcribed from pinned QuPRS
    /// test/test_wmc_direct_cnf_golden.py, never from this solver's output.
    fn golden_text(dialect: Dialect, clauses: &[&str], cancellation: bool) -> String {
        let negative_one = if cancellation {
            ("-1.5", "-0.25")
        } else {
            ("1.5", "-0.25")
        };
        let records = [
            (1, "1.5", "0.25"),
            (-1, negative_one.0, negative_one.1),
            (2, "-0.5", "0.25"),
            (-2, "0.75", "-0.25"),
        ];
        let mut source = format!("p cnf 2 {}\nc t wmc\n", clauses.len());
        for (lit, real, imag) in records {
            let record = match dialect {
                Dialect::Gpmc => format!("c p weight {lit} {real} {imag} 0\n"),
                Dialect::Ganak => {
                    let (sign, magnitude) = if let Some(x) = imag.strip_prefix('-') {
                        ("-", x)
                    } else {
                        ("+", imag)
                    };
                    format!("c p weight {lit} {real} {sign} {magnitude}i 0\n")
                }
            };
            source.push_str(&record);
        }
        for clause in clauses {
            source.push_str(clause);
            source.push('\n');
        }
        source
    }

    fn assert_exact(source: &str, dialect: Dialect, sat: bool, re: BigRational, im: BigRational) {
        let parsed = parse_printed_dimacs(source, dialect).expect("source printed-CNF parser");
        let oracle = independent_exact_full_wmc(&parsed).expect("independent enumeration");
        assert_eq!(oracle.0, sat);
        assert_eq!(oracle.1.real, re);
        assert_eq!(oracle.1.imag, im);
        for cap in [0, 10_000] {
            let report = solve_with_limits(
                &parsed,
                Limits {
                    nodes: 10_000,
                    depth: 128,
                    cache_entries: cap,
                },
            )
            .expect("bounded exact solver on QuPRS source-defined test");
            assert_eq!(report.satisfiable, sat);
            assert_eq!(report.count, oracle.1);
        }
    }

    #[test]
    fn quprs_source_defined_six_golden_cases_in_both_dialects() {
        let cases: [(&str, &[&str], bool, bool, (i64, i64), (i64, i64)); 6] = [
            ("unconstrained", &[], false, true, (3, 4), (0, 1)),
            (
                "forced-sat",
                &["1 0", "-2 0"],
                false,
                true,
                (19, 16),
                (-3, 16),
            ),
            (
                "contradictory-unsat",
                &["1 0", "-1 0"],
                false,
                false,
                (0, 1),
                (0, 1),
            ),
            ("disjunction", &["1 2 0"], false, true, (-5, 16), (9, 16)),
            ("xor", &["1 2 0", "-1 -2 0"], false, true, (1, 2), (5, 16)),
            ("complex-cancellation", &[], true, true, (0, 1), (0, 1)),
        ];
        for dialect in [Dialect::Gpmc, Dialect::Ganak] {
            for (name, clauses, cancellation, sat, (rn, rd), (in_, id)) in cases {
                let source = golden_text(dialect, clauses, cancellation);
                assert_exact(&source, dialect, sat, rat(rn, rd), rat(in_, id));
                if name == "complex-cancellation" {
                    let parsed = parse_printed_dimacs(&source, dialect).unwrap();
                    let (oracle_sat, oracle_wmc) = independent_exact_full_wmc(&parsed).unwrap();
                    assert!(oracle_sat, "SAT may have a zero exact complex count");
                    assert_eq!(oracle_wmc, Gaussian::zero());
                }
            }
        }
    }

    #[test]
    fn quprs_decimal_tseitin_and_unweighted_golden_controls() {
        let forced = "p cnf 1 1\nc t wmc\nc p weight 1 0.6 0.8 0\nc p weight -1 1.0 0.0 0\n1 0\n";
        assert_exact(forced, Dialect::Gpmc, true, rat(3, 5), rat(4, 5));
        let free = "p cnf 2 1\nc t wmc\nc p weight 1 0.5 0 0\nc p weight -1 1 0 0\nc p weight 2 0.5 0 0\nc p weight -2 1 0 0\n1 2 0\n";
        assert_exact(free, Dialect::Gpmc, true, rat(5, 4), rat(0, 1));
        assert_exact(
            "p cnf 2 0\nc t wmc\n",
            Dialect::Gpmc,
            true,
            rat(4, 1),
            rat(0, 1),
        );
        assert_exact(
            "p cnf 1 1\nc t wmc\n0\n",
            Dialect::Gpmc,
            false,
            rat(0, 1),
            rat(0, 1),
        );
        assert_eq!(printed_decimal("1.25e-2").unwrap(), rat(1, 80));
        assert_eq!(printed_decimal("-0.5").unwrap(), rat(-1, 2));
        let scientific = "p cnf 1 1\nc t wmc\nc p weight 1 1e-3 - 2.5e-2i 0\n1 0\n";
        assert_exact(scientific, Dialect::Ganak, true, rat(1, 1000), rat(-1, 40));
    }

    #[test]
    fn malformed_or_ambiguous_quprs_printed_cnf_fails_closed() {
        let invalid_gpmc = [
            "p cnf 2 0\nc p weight 1 1 0 0",
            "p cnf 2 1\nc t wmc\n",
            "p cnf 2 0\nc t wmc\nc p weight 1 1 0 0\nc p weight 1 1 0 0",
            "p cnf 2 1\nc t wmc\n3 0",
            "p cnf 2 1\nc t wmc\n1 0 2 0",
            "p cnf 13 0\nc t wmc",
            "p cnf 1 0\nc t wmc\nc p weight 1 inf 0 0",
            "p cnf 1 0\nc t wmc\nc p weight 1 nan 0 0",
            "p cnf 1 0\nc t wmc\nc p weight 1 1/2 0 0",
            "p cnf 1 0\nc t wmc\nc p weight 1 1e1000 0 0",
            "p cnf 1 0\nc t wmc\nc p weight -1 1 0 0\ngarbage",
            "p cnf 1 0\nc t wmc\nc t wmc",
            "p cnf 1 1\nc t wmc\n1 0\nc p weight 1 0.5 0 0",
            "p cnf 1 1\nc t wmc\n1 0\np cnf 1 1",
        ];
        for source in invalid_gpmc {
            assert!(
                parse_printed_dimacs(source, Dialect::Gpmc).is_err(),
                "must refuse invalid printed input: {source}"
            );
        }
        for token in [
            "nan",
            "inf",
            "1/2",
            "0.1j",
            "1e1000",
            ".",
            "1.2.3",
            "",
            "1".repeat(121).as_str(),
        ] {
            assert!(printed_decimal(token).is_err());
        }
        let ganak_bad = [
            "p cnf 1 0\nc t wmc\nc p weight 1 1 + -0.1i 0",
            "p cnf 1 0\nc t wmc\nc p weight 1 1 + 0.1 0",
            "p cnf 1 0\nc t wmc\nc p weight 1 1 0.1i 0",
            "p cnf 1 0\nc t wmc\nc p weight 1 1 - 0.2i 1",
        ];
        for source in ganak_bad {
            assert!(parse_printed_dimacs(source, Dialect::Ganak).is_err());
        }
        assert!(parse_printed_dimacs(&"z".repeat(MAX_SOURCE_BYTES + 1), Dialect::Gpmc).is_err());
    }

    #[test]
    fn rejects_unknown_unbounded_or_forged_oracle_uses() {
        let input = Problem::new(
            13,
            vec![],
            vec![Weights::new(Gaussian::one(), Gaussian::one()); 13],
        )
        .unwrap();
        assert!(independent_exact_full_wmc(&input).is_err());
        assert!(parse_printed_dimacs("p cnf 0 0\nc t wmc", Dialect::Gpmc).is_err());
    }
}
