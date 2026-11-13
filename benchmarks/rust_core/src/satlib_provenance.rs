//! QOO-219: byte-pinned external SATLIB DIMACS CNFs, with separate synthetic
//! signed/complex exact rational weights. Original CNF provenance is in
//! docs/qoo-219-satlib-provenance.md; do not call these quantum or real WMC inputs.
//!
//! The SAT truth tables below were independently enumerated and are verified
//! exhaustively against the pinned original CNF in the accompanying unit tests.

use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

const UF20_01: &str = include_str!("../fixtures/satlib/uf20-01.cnf");
const UF20_02: &str = include_str!("../fixtures/satlib/uf20-02.cnf");
const DUBOIS20: &str = include_str!("../fixtures/satlib/dubois20.cnf");

const SAT_UF20_01: [u32; 8] = [
    614689, 618529, 618537, 618785, 619017, 619049, 619145, 1009550,
];

const SAT_UF20_02: [u32; 29] = [
    41409, 41425, 57793, 57809, 303296, 303300, 303552, 303553, 303556,
    303568, 303569, 303572, 305616, 305617, 305620, 319680, 319684,
    319936, 319937, 319940, 319952, 319953, 319956, 322000, 322001,
    322004, 322032, 322033, 322036,
];

fn input(name: &str) -> (&'static str, usize, usize) {
    match name {
        "satlib_uf20_01" => (UF20_01, 20, 91),
        "satlib_uf20_02" => (UF20_02, 20, 91),
        "satlib_dubois20_unverified" => (DUBOIS20, 60, 160),
        _ => panic!("only explicitly source-pinned SATLIB instance names"),
    }
}

fn parse_original_dimacs(name: &str) -> (usize, Vec<Vec<i32>>) {
    let (text, expected_variables, expected_clauses) = input(name);
    let mut header = None;
    let mut clauses = Vec::new();
    let mut current = Vec::new();
    let mut ended = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('c') {
            continue;
        }
        if trimmed.starts_with('%') {
            ended = true;
            break;
        }
        if trimmed.starts_with('p') {
            assert!(header.is_none(), "one DIMACS header expected");
            let parts: Vec<_> = trimmed.split_whitespace().collect();
            assert_eq!(parts.len(), 4, "DIMACS header length");
            assert_eq!(parts[0], "p");
            assert_eq!(parts[1], "cnf");
            header = Some((
                parts[2].parse::<usize>().expect("DIMACS variable count"),
                parts[3].parse::<usize>().expect("DIMACS clause count"),
            ));
            continue;
        }
        assert!(header.is_some(), "DIMACS clauses require header");
        for token in trimmed.split_whitespace() {
            let lit = token.parse::<i32>().expect("DIMACS literal integer");
            if lit == 0 {
                assert!(!current.is_empty(), "source has no empty clauses");
                clauses.push(std::mem::take(&mut current));
            } else {
                assert!((lit.unsigned_abs() as usize) <= expected_variables);
                current.push(lit);
            }
        }
    }
    let (variables, declared) = header.expect("p cnf header exists");
    assert_eq!(variables, expected_variables, "provenance variable count");
    assert_eq!(declared, expected_clauses, "provenance clause header");
    assert_eq!(clauses.len(), expected_clauses, "all clauses accounted for");
    assert!(current.is_empty(), "each DIMACS clause must end in 0");
    let _ = ended; // original SATLIB files may use a % end marker.
    (variables, clauses)
}

fn weight_for_variable(index: usize) -> Weights {
    // These weights are entirely synthetic, deliberately heterogeneous and
    // not present in SATLIB. Keep fixed even when timing results disappoint.
    let k = index as i64 + 1;
    let q = |n: i64, d: i64| BigRational::new(n.into(), d.into());
    Weights::new(
        Gaussian::new(q(k % 7 - 3, k + 2), q(k % 5 - 2, 2 * k + 1)),
        Gaussian::new(q(k % 5 - 3, k + 3), q(k % 11 - 5, k + 4)),
    )
}

pub fn fixture(name: &str) -> Problem {
    let (n, clauses) = parse_original_dimacs(name);
    let weights = (0..n).map(weight_for_variable).collect();
    Problem::new(n, clauses, weights).expect("original DIMACS is well-formed")
}

fn schoolbook_multiply(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

fn schoolbook_add(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(&a.real + &b.real, &a.imag + &b.imag)
}

fn satisfies(clauses: &[Vec<i32>], assignment: u32) -> bool {
    clauses.iter().all(|clause| {
        clause.iter().any(|lit| {
            let selected = assignment & (1_u32 << (lit.unsigned_abs() - 1)) != 0;
            if *lit > 0 { selected } else { !selected }
        })
    })
}

/// Independent SAT truth tables, verified by exhaustive 2^20 enumeration.
/// WMC is a direct sum of four-real-product exact Gaussian assignment weights;
/// no DPLL solver, solver cache, or public Gaussian::mul are used.
pub fn independent_full_wmc(name: &str, problem: &Problem) -> (bool, Gaussian) {
    let masks: &[u32] = match name {
        "satlib_uf20_01" => &SAT_UF20_01,
        "satlib_uf20_02" => &SAT_UF20_02,
        _ => panic!("no independently established exact SAT mask oracle"),
    };
    assert_eq!(problem.variables(), 20, "20-variable SATLIB oracle");
    let mut total = Gaussian::zero();
    for &mask in masks {
        assert!(satisfies(problem.clauses(), mask), "stored SAT witness");
        let mut product = Gaussian::one();
        for (i, weight) in problem.weights().iter().enumerate() {
            let v = if mask & (1_u32 << i) != 0 {
                &weight.positive
            } else {
                &weight.negative
            };
            product = schoolbook_multiply(&product, v);
        }
        total = schoolbook_add(&total, &product);
    }
    (!masks.is_empty(), total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{solve_with_limits, Limits};

    #[test]
    fn original_satlib_source_is_parsed_without_clause_edits() {
        for name in ["satlib_uf20_01", "satlib_uf20_02", "satlib_dubois20_unverified"] {
            let p = fixture(name);
            assert_eq!(p.clauses().len(), input(name).2);
            assert_eq!(p.variables(), input(name).1);
        }
    }

    #[test]
    fn independent_truth_tables_are_complete_for_satisfiable_sources() {
        for (name, expected_count) in [("satlib_uf20_01", 8), ("satlib_uf20_02", 29)] {
            let p = fixture(name);
            let mut actual = Vec::new();
            for mask in 0_u32..(1_u32 << 20) {
                if satisfies(p.clauses(), mask) {
                    actual.push(mask);
                }
            }
            let expected: &[u32] = if name == "satlib_uf20_01" {
                &SAT_UF20_01
            } else {
                &SAT_UF20_02
            };
            assert_eq!(actual.len(), expected_count);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn exact_complex_weighted_counts_match_independent_truth_tables() {
        for name in ["satlib_uf20_01", "satlib_uf20_02"] {
            let p = fixture(name);
            let (sat, expected) = independent_full_wmc(name, &p);
            assert!(sat);
            for cache_entries in [0_usize, 10_000_usize] {
                let report = solve_with_limits(
                    &p,
                    Limits { cache_entries, ..Limits::default() },
                )
                .expect("SATLIB bounded Full WMC solve");
                assert_eq!(report.satisfiable, sat, "Boolean SAT is separate");
                assert_eq!(report.count, expected, "independent schoolbook Full WMC");
            }
        }
    }
}
