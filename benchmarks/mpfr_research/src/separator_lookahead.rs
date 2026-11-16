//! Finite cofactor separator lookahead directly on original weighted CNF.
//! The algorithm ONLY chooses a DPLL branch variable; the Shannon identity
//! WMC(F) = w(v) WMC(F[v=true]) + w(!v) WMC(F[v=false])
//! is unchanged. No SAT-only elimination or native QuPRS ANF is used.

use crate::Clause;

const MAX_SCANS: usize = 320;
const MAX_WORK: usize = 4_000_000;
const CANDIDATES: usize = 8;

#[derive(Default, Clone, Debug)]
pub(crate) struct SeparatorStats {
    pub(crate) scans: usize,
    pub(crate) candidates: usize,
    pub(crate) chosen: usize,
    pub(crate) no_split: usize,
    pub(crate) work: usize,
    pub(crate) censored: bool,
}

#[derive(Clone, Copy)]
struct Projection {
    largest: usize,
    components: usize,
    contradiction: bool,
}

fn root(parent: &mut [usize], mut v: usize) -> usize {
    while parent[v] != v {
        parent[v] = parent[parent[v]];
        v = parent[v];
    }
    v
}

/// Compute connected components of a hypothetical single-variable
/// cofactor without changing clauses or weights. Unit propagation can
/// produce even more separation; this projection is conservative.
fn cofactor(
    clauses: &[Clause],
    v: usize,
    truth: bool,
    n: usize,
    stats: &mut SeparatorStats,
) -> Option<Projection> {
    let selected = if truth { v as i32 } else { -(v as i32) };
    let opposite = -selected;
    let mut parent: Vec<_> = (0..=n).collect();
    let mut used = vec![false; n + 1];
    for clause in clauses {
        let mut satisfied = false;
        for &lit in clause {
            if stats.work >= MAX_WORK {
                stats.censored = true;
                return None;
            }
            stats.work += 1;
            if lit == selected {
                satisfied = true;
                break;
            }
        }
        if satisfied {
            continue;
        }
        let mut first = None;
        for &lit in clause {
            if stats.work >= MAX_WORK {
                stats.censored = true;
                return None;
            }
            stats.work += 1;
            if lit == opposite {
                continue;
            }
            let var = lit.unsigned_abs() as usize;
            used[var] = true;
            if let Some(f) = first {
                let a = root(&mut parent, f);
                let b = root(&mut parent, var);
                parent[b] = a;
            } else {
                first = Some(var);
            }
        }
        if first.is_none() {
            return Some(Projection {
                largest: 0,
                components: 0,
                contradiction: true,
            });
        }
    }
    let mut count = vec![0_usize; n + 1];
    for (v, &yes) in used.iter().enumerate().skip(1) {
        if yes {
            count[root(&mut parent, v)] += 1;
        }
    }
    Some(Projection {
        largest: count.iter().copied().max().unwrap_or(0),
        components: count.iter().filter(|&&k| k > 0).count(),
        contradiction: false,
    })
}

/// Bounded lookahead for up to eight high-frequency candidates. Select a
/// pivot ONLY when at least one cofactor splits or is contradictory; any
/// negative admission or exhausted work preserves baseline Frequency.
pub(crate) fn choose(
    clauses: &[Clause],
    active: &[usize],
    frequency: &[usize],
    n: usize,
    stats: &mut SeparatorStats,
) -> Option<usize> {
    if active.len() < 4
        || active.len() > 640
        || clauses.is_empty()
        || clauses.len() > 1300
        || stats.scans >= MAX_SCANS
        || stats.work >= MAX_WORK
        || clauses.iter().map(Vec::len).sum::<usize>() > 7000
    {
        return None;
    }
    stats.scans += 1;
    let mut candidates = active.to_vec();
    candidates.sort_unstable_by_key(|&v| (std::cmp::Reverse(frequency[v]), v));
    let mut best: Option<(usize, (usize, usize, usize))> = None;
    for v in candidates.into_iter().take(CANDIDATES) {
        stats.candidates += 1;
        let yes = cofactor(clauses, v, true, n, stats)?;
        let no = cofactor(clauses, v, false, n, stats)?;
        if !yes.contradiction && !no.contradiction && yes.components <= 1 && no.components <= 1 {
            continue;
        }
        // Favor balanced projected largest components rather than
        // fragile one-sided branch reductions. Tie-break deterministically.
        let score = (
            yes.largest.max(no.largest),
            yes.largest + no.largest,
            usize::MAX - frequency[v],
        );
        if best
            .as_ref()
            .is_none_or(|&(old_v, old)| (score, v) < (old, old_v))
        {
            best = Some((v, score));
        }
    }
    if let Some((v, _)) = best {
        stats.chosen += 1;
        Some(v)
    } else {
        stats.no_split += 1;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, CachePolicy, Engine};

    #[test]
    fn chooses_bridge_variable_from_original_cnf_cofactors() {
        let cnf: Vec<Clause> = vec![
            vec![1, 2],
            vec![1, 3],
            vec![-1, 4],
            vec![-2, 5],
            vec![-3, 6],
            vec![4, 7],
        ];
        let mut frequency = vec![0_usize; 8];
        for clause in &cnf {
            for &lit in clause {
                frequency[lit.unsigned_abs() as usize] += 1;
            }
        }
        let mut stats = SeparatorStats::default();
        assert_eq!(
            choose(
                &cnf,
                &(1..=7).collect::<Vec<_>>(),
                &frequency,
                7,
                &mut stats
            ),
            Some(1)
        );
        assert_eq!(stats.chosen, 1);
    }

    #[test]
    fn sparse_small_weighted_wmc_matches_frequency_at_both_mpfr_widths() {
        for bits in [100, 333] {
            for seed in 0..40_usize {
                let mut input = String::from("p cnf 7 6\nc t wmc\n");
                for v in 1..=7 {
                    input.push_str(&format!("c p weight {v} 0.25 0.125 0\n"));
                    input.push_str(&format!("c p weight -{v} -0.125 0.25 0\n"));
                }
                for (i, clause) in [[1, 2], [1, 3], [-1, 4], [-2, 5], [-3, 6], [4, 7]]
                    .iter()
                    .enumerate()
                {
                    let (a, b) = (clause[0], clause[1]);
                    let (a, b) = if seed & (1 << (i % 5)) != 0 {
                        (-a, b)
                    } else {
                        (a, b)
                    };
                    input.push_str(&format!("{a} {b} 0\n"));
                }
                let p = parse_emitted(&input, bits).unwrap();
                let mut reference = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                reference.reuse_frequency_scratch = true;
                let original = reference
                    .count(p.clauses.clone(), (1..=7).collect(), 0)
                    .unwrap();
                let mut new = Engine::new(&p, bits, CachePolicy::HbBorrowed);
                new.reuse_frequency_scratch = true;
                new.separator_lookahead = true;
                let alternative = new.count(p.clauses.clone(), (1..=7).collect(), 0).unwrap();
                assert_eq!(
                    (original.sat, original.count.re, original.count.im),
                    (alternative.sat, alternative.count.re, alternative.count.im),
                    "seed={seed}, bits={bits}"
                );
            }
        }
    }

    #[test]
    fn work_cap_falls_back_to_unmodified_frequency() {
        let cnf: Vec<Clause> = vec![vec![1, 2], vec![1, 3], vec![-1, 4], vec![-2, 5]];
        let scope: Vec<usize> = (1..=5).collect();
        let mut frequency = vec![0; 6];
        for clause in &cnf {
            for &lit in clause {
                frequency[lit.unsigned_abs() as usize] += 1;
            }
        }
        let mut stats = SeparatorStats {
            work: MAX_WORK - 1,
            ..Default::default()
        };
        assert!(choose(&cnf, &scope, &frequency, 5, &mut stats).is_none());
        assert!(stats.censored);
        assert!(choose(&cnf, &scope, &frequency, 5, &mut stats).is_none());
    }
}
