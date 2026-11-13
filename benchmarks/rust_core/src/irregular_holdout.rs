//! Predeclared irregular synthetic CNFs with independently enumerated Full WMC.
//!
//! Both fixtures are NEW in QOO-215 and are not claimed to be paper-derived,
//! realistic industrial problems, or QuPRS end-to-end inputs. The irregular
//! incidence and heterogeneous signed/complex rational weights stress paths
//! that chain, pair, cycle and forced-variable controls do not exercise.
//! Do not tune an admission policy on these and then call them independent.

use num_rational::BigRational;
use rawmocs_core::{Gaussian, Problem, Weights};

fn ratio(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn weights(variables: usize) -> Vec<Weights> {
    (0..variables)
        .map(|index| {
            let k = index as i64 + 1;
            Weights::new(
                Gaussian::new(ratio(k % 5 + 1, k + 2), ratio(k % 7 - 3, k + 3)),
                Gaussian::new(ratio(k % 3 - 1, 2 * k + 1), ratio(k % 5 - 2, k + 4)),
            )
        })
        .collect()
}

/// The clause tables are intentionally fixed, with heterogeneous widths,
/// degrees, signs and connected incidence. Do not generate randomized CNFs.
pub fn irregular(name: &str) -> Problem {
    let (n, clauses): (usize, Vec<Vec<i32>>) = match name {
        "irregular8" => (
            8,
            vec![
                vec![1, -3, 5],
                vec![-1, 4],
                vec![2, -5, 7],
                vec![-2, 6],
                vec![-4, -6, 8],
                vec![3, -7],
                vec![-8, 5],
                vec![-5, 2, -7],
                vec![4, -2],
                vec![-3, 8],
                vec![-6, 1, 7],
            ],
        ),
        "irregular10" => (
            10,
            vec![
                vec![1, 4, -7],
                vec![-1, 2],
                vec![-2, 5, 9],
                vec![3, -5],
                vec![-3, 6, 10],
                vec![-6, 7],
                vec![2, -8, 10],
                vec![-9, 4],
                vec![8, -10],
                vec![-4, 6, 9],
                vec![1, -5, 8],
                vec![-7, -8, 3],
                vec![-2, -4, 10],
                vec![-1, -3, 9],
            ],
        ),
        _ => panic!("only source-declared irregular8 or irregular10"),
    };
    Problem::new(n, clauses, weights(n)).expect("fixed valid irregular CNF")
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

/// Exhaustive Boolean truth-table and 4-real-product exact Gaussian WMC.
/// No solver, cache or Gaussian::mul is invoked in this oracle.
pub fn independent_exact_irregular_wmc(problem: &Problem) -> (bool, Gaussian) {
    let variables = problem.variables();
    assert!((1..=10).contains(&variables), "bounded independent oracle");
    let mut satisfiable = false;
    let mut total = Gaussian::zero();
    for assignment in 0_usize..(1_usize << variables) {
        let sat = problem.clauses().iter().all(|clause| {
            clause.iter().any(|&lit| {
                let index = lit.unsigned_abs() as usize - 1;
                let selected = assignment & (1_usize << index) != 0;
                if lit > 0 {
                    selected
                } else {
                    !selected
                }
            })
        });
        if !sat {
            continue;
        }
        satisfiable = true;
        let mut product = Gaussian::one();
        for (index, weight) in problem.weights().iter().enumerate() {
            let selected = assignment & (1_usize << index) != 0;
            let factor = if selected {
                &weight.positive
            } else {
                &weight.negative
            };
            product = schoolbook_multiply(&product, factor);
        }
        total = schoolbook_add(&total, &product);
    }
    (satisfiable, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawmocs_core::{solve_with_limits, Limits};

    #[test]
    fn irregular_holdouts_agree_with_independent_exact_oracle() {
        for name in ["irregular8", "irregular10"] {
            let input = irregular(name);
            let (satisfiable, expected) = independent_exact_irregular_wmc(&input);
            assert!(satisfiable, "{name} must contain satisfying assignments");
            for cap in [0, 10_000] {
                let report = solve_with_limits(
                    &input,
                    Limits {
                        cache_entries: cap,
                        ..Limits::default()
                    },
                )
                .expect("fixed bounded irregular full WMC");
                assert_eq!(report.satisfiable, satisfiable, "{name} cache={cap}");
                assert_eq!(report.count, expected, "{name} cache={cap}");
            }
        }
    }
}
