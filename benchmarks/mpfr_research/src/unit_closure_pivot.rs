//! Research-only bounded unit-closure-aware pivot selection on the original Weighted CNF.
//! Projection performs Boolean unit propagation without changing the actual
//! weighted DPLL recursion, signed literal weights, component cache, or CNF.
//! Budget exhaustion fails closed to the baseline Frequency pivot.

use crate::Clause;
use std::cmp::Reverse;

const MAX_SCANS: usize = 64;
const MAX_WORK: usize = 500_000;
const MAX_CANDIDATES: usize = 6;
const MAX_ACTIVE: usize = 640;
const MAX_CLAUSES: usize = 1_300;
const MAX_LITERALS: usize = 7_000;

#[derive(Clone, Default, Debug)]
pub(crate) struct ClosureStats {
    pub(crate) scans: usize,
    pub(crate) candidates: usize,
    pub(crate) chosen: usize,
    pub(crate) rejected: usize,
    pub(crate) work: usize,
    pub(crate) forced: usize,
    pub(crate) contradictions: usize,
    pub(crate) censored: bool,
}

#[derive(Clone, Copy, Debug)]
struct Projection {
    largest: usize,
    components: usize,
    forced: usize,
    contradiction: bool,
}

fn consume(stats: &mut ClosureStats) -> Option<()> {
    if stats.work >= MAX_WORK {
        stats.censored = true;
        None
    } else {
        stats.work += 1;
        Some(())
    }
}

fn root(parent: &mut [usize], mut variable: usize) -> usize {
    while parent[variable] != variable {
        parent[variable] = parent[parent[variable]];
        variable = parent[variable];
    }
    variable
}

/// Simulate one Shannon branch and all *Boolean* implied unit literals.
/// A contradiction here is logical UNSAT of that cofactor, never a
/// conclusion from a zero complex weighted count. The result is only
/// a score; the real DPLL still performs every weighted operation.
fn project(
    clauses: &[Clause],
    variable: usize,
    truth: bool,
    n: usize,
    stats: &mut ClosureStats,
) -> Option<Projection> {
    let mut assigned = vec![0_i8; n + 1];
    assigned[variable] = if truth { 1 } else { -1 };
    let mut forced = 0_usize;
    loop {
        let mut unit = None;
        for clause in clauses {
            let mut satisfied = false;
            let mut open = 0_usize;
            let mut last = 0_i32;
            for &lit in clause {
                consume(stats)?;
                let sign = if lit > 0 { 1_i8 } else { -1_i8 };
                match assigned[lit.unsigned_abs() as usize] {
                    0 => {
                        open += 1;
                        last = lit;
                    }
                    value if value == sign => {
                        satisfied = true;
                        break;
                    }
                    _ => {}
                }
            }
            if satisfied {
                continue;
            }
            if open == 0 {
                stats.contradictions += 1;
                return Some(Projection {
                    largest: 0,
                    components: 0,
                    forced,
                    contradiction: true,
                });
            }
            if open == 1 {
                unit = Some(last);
                break;
            }
        }
        let Some(lit) = unit else { break };
        let variable = lit.unsigned_abs() as usize;
        let sign = if lit > 0 { 1_i8 } else { -1_i8 };
        if assigned[variable] != 0 {
            // A non-satisfied, single-open clause must be unassigned.
            // Treat impossible internal inconsistency as a failed probe.
            stats.censored = true;
            return None;
        }
        assigned[variable] = sign;
        forced += 1;
    }

    // Compute variable-incidence components of the *propagated* cofactor.
    // Clauses satisfied by any assigned literal disappear; falsified
    // literals disappear but no original clause or weight is mutated.
    let mut parent: Vec<usize> = (0..=n).collect();
    let mut used = vec![false; n + 1];
    for clause in clauses {
        let mut satisfied = false;
        for &lit in clause {
            consume(stats)?;
            let sign = if lit > 0 { 1_i8 } else { -1_i8 };
            if assigned[lit.unsigned_abs() as usize] == sign {
                satisfied = true;
                break;
            }
        }
        if satisfied {
            continue;
        }
        let mut first = None;
        for &lit in clause {
            consume(stats)?;
            let variable = lit.unsigned_abs() as usize;
            if assigned[variable] != 0 {
                continue;
            }
            used[variable] = true;
            if let Some(first_variable) = first {
                let a = root(&mut parent, first_variable);
                let b = root(&mut parent, variable);
                parent[b] = a;
            } else {
                first = Some(variable);
            }
        }
    }
    let mut size = vec![0_usize; n + 1];
    for (variable, &present) in used.iter().enumerate().skip(1) {
        if present {
            size[root(&mut parent, variable)] += 1;
        }
    }
    Some(Projection {
        largest: size.iter().copied().max().unwrap_or(0),
        components: size.iter().filter(|&&v| v != 0).count(),
        forced,
        contradiction: false,
    })
}

fn worst(a: Projection, b: Projection) -> usize {
    a.largest.max(b.largest)
}

/// Reject unless both branches demonstrably break into multiple independent
/// components, or one is Boolean contradictory and the other splits.
/// Also require a >=20% improvement in worst retained component over the
/// *actual* Frequency cofactor, with at least two variables of margin.
fn worth_switching(
    baseline: (Projection, Projection),
    proposed: (Projection, Projection),
) -> bool {
    let (yes, no) = proposed;
    let meaningful_split = (yes.components >= 2 && no.components >= 2)
        || (yes.contradiction && no.components >= 2)
        || (no.contradiction && yes.components >= 2);
    let before = worst(baseline.0, baseline.1);
    let after = worst(yes, no);
    meaningful_split
        && before >= 12
        && after.saturating_add((before / 5).max(2)) <= before
}

/// Scan a bounded set of source-CNF high-frequency branch candidates.
/// No successful scoring result is used if any required probe is censored.
pub(crate) fn choose(
    clauses: &[Clause],
    active: &[usize],
    frequency: &[usize],
    n: usize,
    stats: &mut ClosureStats,
) -> Option<usize> {
    if stats.censored
        || stats.scans >= MAX_SCANS
        || stats.work >= MAX_WORK
        || active.len() < 6
        || active.len() > MAX_ACTIVE
        || clauses.len() > MAX_CLAUSES
        || clauses.is_empty()
        || clauses.iter().map(Vec::len).sum::<usize>() > MAX_LITERALS
    {
        return None;
    }
    let baseline = *active.iter().max_by_key(|&&v| frequency[v])?;
    stats.scans += 1;
    let first = project(clauses, baseline, true, n, stats)?;
    let second = project(clauses, baseline, false, n, stats)?;
    let reference = (first, second);
    stats.forced += first.forced + second.forced;
    let mut candidates = active.to_vec();
    candidates.sort_unstable_by_key(|&v| (Reverse(frequency[v]), Reverse(v)));
    let mut best: Option<(usize, (usize, usize, usize))> = None;
    for variable in candidates.into_iter().take(MAX_CANDIDATES) {
        if variable == baseline {
            continue;
        }
        stats.candidates += 1;
        let yes = project(clauses, variable, true, n, stats)?;
        let no = project(clauses, variable, false, n, stats)?;
        stats.forced += yes.forced + no.forced;
        if !worth_switching(reference, (yes, no)) {
            continue;
        }
        let score = (
            worst(yes, no),
            yes.largest.saturating_add(no.largest),
            usize::MAX - frequency[variable],
        );
        if best.as_ref().is_none_or(|&(old, old_score)| {
            (score, variable) < (old_score, old)
        }) {
            best = Some((variable, score));
        }
    }
    if let Some((pivot, _)) = best {
        stats.chosen += 1;
        Some(pivot)
    } else {
        stats.rejected += 1;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, CachePolicy, Engine};

    #[test]
    fn unit_closure_exposes_components_hidden_by_single_cofactor() {
        // 1=true forces 2=true and 1=false forces 2=false.
        // Without unit propagation, the two groups connect through 2.
        let cnf: Vec<Clause> = vec![
            vec![-1, 2], vec![1, -2],
            vec![2, 3, 4], vec![-2, 5, 6],
            vec![3, 4], vec![5, 6],
        ];
        let mut stats = ClosureStats::default();
        let yes = project(&cnf, 1, true, 6, &mut stats).unwrap();
        let no = project(&cnf, 1, false, 6, &mut stats).unwrap();
        assert!(yes.forced >= 1 && no.forced >= 1);
        assert_eq!((yes.components, no.components), (2, 2));
        assert_eq!((yes.largest, no.largest), (2, 2));
        assert!(!yes.contradiction && !no.contradiction);
    }

    #[test]
    fn conflict_is_boolean_not_numerical_cancellation() {
        let cnf: Vec<Clause> = vec![vec![-1, 2], vec![-2], vec![3, 4]];
        let mut stats = ClosureStats::default();
        let yes = project(&cnf, 1, true, 4, &mut stats).unwrap();
        assert!(yes.contradiction);
        assert!(stats.contradictions > 0);
    }

    #[test]
    fn budgets_and_small_negative_controls_fall_back() {
        let cnf: Vec<Clause> = vec![
            vec![1, 2], vec![-1, 3], vec![-3, 4],
            vec![-4, 5], vec![-5, 6], vec![-6, 1],
        ];
        let frequency = vec![0, 2, 1, 2, 2, 2, 2];
        let mut stats = ClosureStats {
            work: MAX_WORK - 1,
            ..Default::default()
        };
        assert!(choose(&cnf, &(1..=6).collect::<Vec<_>>(), &frequency, 6, &mut stats).is_none());
        assert!(stats.censored);
        let unchanged_work = stats.work;
        assert!(choose(&cnf, &(1..=6).collect::<Vec<_>>(), &frequency, 6, &mut stats).is_none());
        assert_eq!(stats.work, unchanged_work);
        let mut small = ClosureStats::default();
        assert!(choose(&cnf, &[1, 2, 3], &frequency, 6, &mut small).is_none());
        assert_eq!(small.work, 0);
    }

    #[test]
    fn weighted_full_complex_wmc_matches_frequency_at_100_and_333_bits() {
        // Dyadic complex weights are represented exactly at both widths.
        // Covers SAT, mixed polarity, zero-sum and Boolean UNSAT cases.
        let fixtures = [
            "p cnf 6 6\nc t wmc\nc p weight 1 0 0 0\nc p weight -1 0 0 0\n-1 2 0\n1 -2 0\n2 3 4 0\n-2 5 6 0\n3 4 0\n5 6 0\n",
            "p cnf 6 8\nc t wmc\n1 2 3 0\n-1 4 0\n-2 -3 0\n-4 5 0\n-5 6 0\n-6 1 0\n2 -6 0\n-2 4 0\n",
            "p cnf 6 2\nc t wmc\n1 0\n-1 0\n",
        ];
        for bits in [100_u32, 333_u32] {
            for (case, fixture) in fixtures.iter().enumerate() {
                let mut p = parse_emitted(fixture, bits).unwrap();
                if case == 1 {
                    for (i, weight) in p.weights.iter_mut().enumerate() {
                        weight.positive = crate::C::decimal(
                            bits, if i % 2 == 0 { "0.25" } else { "-0.5" }, "0.125"
                        ).unwrap();
                        weight.negative = crate::C::decimal(bits, "-0.125", "0.25").unwrap();
                    }
                }
                let mut base = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                let expected = base.count(p.clauses.clone(), (1..=6).collect(), 0).unwrap();
                let mut alternative = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                alternative.unit_closure_pivot = true;
                let actual = alternative.count(p.clauses.clone(), (1..=6).collect(), 0).unwrap();
                assert_eq!(
                    (actual.sat, actual.count.re, actual.count.im),
                    (expected.sat, expected.count.re, expected.count.im),
                    "case={case} bits={bits}"
                );
            }
        }
    }
}
