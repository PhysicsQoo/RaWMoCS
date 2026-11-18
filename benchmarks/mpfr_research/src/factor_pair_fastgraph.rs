//! Research-only bitset primal graph for *bounded* two-variable cutset admission.
//! One source-CNF graph build per input; every pair probe clones only small
//! 320-vertex bitset rows. This reproduces the existing feasible-width-first
//! min-fill order/guard exactly, with no CNF or complex-weight rewrite.
use crate::{factor_minfill_order, Clause};

pub(crate) const MAX_VERTICES: usize = 320;
const MAX_WORDS: usize = 5; // 5 * 64 = 320.

#[derive(Clone)]
pub(crate) struct FastGraph {
    vars: Vec<usize>,
    rows: Vec<[u64; MAX_WORDS]>,
    degrees: Vec<usize>,
    words: usize,
}

impl FastGraph {
    pub(crate) fn build(clauses: &[Clause], scope: &[usize]) -> Option<Self> {
        if scope.is_empty() || scope.len() > MAX_VERTICES { return None; }
        let mut vars = scope.to_vec();
        vars.sort_unstable();
        if vars[0] == 0 || vars.windows(2).any(|w| w[0] == w[1]) {
            return None;
        }
        let n = vars.len();
        let words = n.div_ceil(64);
        let mut position = vec![usize::MAX; vars[n - 1] + 1];
        for (i, &v) in vars.iter().enumerate() { position[v] = i; }
        let mut rows = vec![[0_u64; MAX_WORDS]; n];
        for clause in clauses {
            let mut indices = Vec::with_capacity(clause.len());
            for &literal in clause {
                let v = literal.unsigned_abs() as usize;
                if v == 0 || v >= position.len() || position[v] == usize::MAX {
                    return None;
                }
                indices.push(position[v]);
            }
            indices.sort_unstable();
            indices.dedup();
            for (i, &a) in indices.iter().enumerate() {
                for &b in &indices[i + 1..] {
                    rows[a][b / 64] |= 1_u64 << (b % 64);
                    rows[b][a / 64] |= 1_u64 << (a % 64);
                }
            }
        }
        let degrees = rows.iter().map(|row| {
            row[..words].iter().map(|w| w.count_ones() as usize).sum()
        }).collect();
        Some(Self { vars, rows, degrees, words })
    }

    pub(crate) fn ranked(&self, max_candidates: usize) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.vars.len()).collect();
        order.sort_unstable_by_key(|&i| {
            (std::cmp::Reverse(self.degrees[i]), self.vars[i])
        });
        order.into_iter().take(max_candidates)
            .map(|i| self.vars[i]).collect()
    }

    /// Exactly the same min-fill priority and width admission as
    /// factor_minfill_order::plan on the original graph with x,y deleted.
    /// No complex MPFR operations and no clauses/weights copied per probe.
    pub(crate) fn plan_deleted(
        &self, x: usize, y: usize, max_bucket: usize,
    ) -> factor_minfill_order::FillPlan {
        let mut out = factor_minfill_order::FillPlan::default();
        let n = self.vars.len();
        if x == y || max_bucket == 0 || !self.vars.contains(&x)
            || !self.vars.contains(&y) || n < 3 {
            out.capped = true;
            return out;
        }
        let xi = self.vars.binary_search(&x).unwrap();
        let yi = self.vars.binary_search(&y).unwrap();
        let mut active = [0_u64; MAX_WORDS];
        for i in 0..n { active[i / 64] |= 1_u64 << (i % 64); }
        active[xi / 64] &= !(1_u64 << (xi % 64));
        active[yi / 64] &= !(1_u64 << (yi % 64));
        let mut rows = self.rows.clone();

        for _ in 0..n - 2 {
            let mut best: Option<(bool, usize, usize, usize, usize)> = None;
            for i in 0..n {
                if active[i / 64] & (1_u64 << (i % 64)) == 0 { continue; }
                let mut neighbors = [0_u64; MAX_WORDS];
                let mut degree = 0_usize;
                for w in 0..self.words {
                    neighbors[w] = rows[i][w] & active[w];
                    degree += neighbors[w].count_ones() as usize;
                }
                let bucket = degree + 1;
                // Each existing edge among these neighbors is counted twice.
                let mut twice_edges = 0_usize;
                for (w, &bits) in neighbors.iter().take(self.words).enumerate() {
                    let mut pending = bits;
                    while pending != 0 {
                        let j = w * 64 + pending.trailing_zeros() as usize;
                        pending &= pending - 1;
                        for t in 0..self.words {
                            twice_edges += (rows[j][t] & neighbors[t]).count_ones() as usize;
                        }
                    }
                }
                debug_assert_eq!(twice_edges % 2, 0);
                let fill = degree * (degree - 1) / 2 - twice_edges / 2;
                out.candidate_evaluations += 1;
                let key = (bucket > max_bucket, fill, bucket, self.vars[i], i);
                if best.as_ref().is_none_or(|old| key < *old) {
                    best = Some(key);
                }
            }
            let (over, fill, bucket, variable, i) = best.expect("remaining active variable");
            out.max_bucket = out.max_bucket.max(bucket);
            if over {
                out.capped = true;
                return out;
            }
            let mut neighbors = [0_u64; MAX_WORDS];
            for w in 0..self.words { neighbors[w] = rows[i][w] & active[w]; }

            // Fill the eliminated variable's neighbor clique, exactly
            // matching original Dense min-fill. Edges to removed vertices
            // are masked at every subsequent active-neighbor query.
            for (w, &bits) in neighbors.iter().take(self.words).enumerate() {
                let mut pending = bits;
                while pending != 0 {
                    let j = w * 64 + pending.trailing_zeros() as usize;
                    pending &= pending - 1;
                    for (t, &mask) in neighbors.iter().take(self.words).enumerate() {
                        rows[j][t] |= mask;
                    }
                    rows[j][j / 64] &= !(1_u64 << (j % 64));
                }
            }
            out.fill_edges += fill;
            out.order.push(variable);
            active[i / 64] &= !(1_u64 << (i % 64));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deleted_clauses(clauses: &[Clause], x: usize, y: usize) -> Vec<Clause> {
        clauses.iter().map(|c| c.iter().copied().filter(|lit| {
            let v = lit.unsigned_abs() as usize;
            v != x && v != y
        }).collect()).collect()
    }

    fn compare(clauses: &[Clause], scope: &[usize], cap: usize) {
        let graph = FastGraph::build(clauses, scope).unwrap();
        for (a, &x) in scope.iter().enumerate() {
            for &y in &scope[a + 1..] {
                let rest = scope.iter().copied().filter(|&v| v != x && v != y)
                    .collect::<Vec<_>>();
                let expected = factor_minfill_order::plan(
                    &deleted_clauses(clauses, x, y), &rest, cap);
                let got = graph.plan_deleted(x, y, cap);
                assert_eq!(got.capped, expected.capped, "x={x} y={y} cap={cap}");
                assert_eq!(got.max_bucket, expected.max_bucket, "x={x} y={y} cap={cap}");
                assert_eq!(got.fill_edges, expected.fill_edges, "x={x} y={y} cap={cap}");
                assert_eq!(got.order, expected.order, "x={x} y={y} cap={cap}");
                assert_eq!(got.candidate_evaluations,
                           expected.candidate_evaluations, "x={x} y={y} cap={cap}");
            }
        }
    }

    #[test]
    fn exact_same_planner_exhaustive_all_five_vertex_graphs() {
        let scope = [1, 2, 3, 4, 5];
        let mut possible = Vec::new();
        for i in 1..=5 {
            for j in i + 1..=5 { possible.push(vec![i, j]); }
        }
        for mask in 0..(1_usize << possible.len()) {
            let clauses = possible.iter().enumerate().filter_map(|(i, e)| {
                if mask & (1 << i) != 0 { Some(e.clone()) } else { None }
            }).collect::<Vec<Clause>>();
            for cap in [1, 2, 3, 11] {
                compare(&clauses, &scope, cap);
            }
        }
    }

    #[test]
    fn same_planner_on_nontrivial_large_sparse_and_dense_cnfs() {
        for n in [16_usize, 64, 128, 285, 320] {
            let scope = (1..=n).collect::<Vec<usize>>();
            let mut clauses = Vec::new();
            for i in 1..=n {
                for offset in [1, 2, 5, 11] {
                    clauses.push(vec![i as i32, ((i + offset - 1) % n + 1) as i32]);
                }
            }
            let fast = FastGraph::build(&clauses, &scope).unwrap();
            let ranked = fast.ranked(4);
            for (i, &x) in ranked.iter().enumerate() {
                for &y in &ranked[i + 1..] {
                    let rest = scope.iter().copied()
                        .filter(|&v| v != x && v != y).collect::<Vec<_>>();
                    let old = factor_minfill_order::plan(
                        &deleted_clauses(&clauses, x, y), &rest, 11);
                    let got = fast.plan_deleted(x, y, 11);
                    assert_eq!(got.capped, old.capped);
                    assert_eq!(got.order, old.order);
                    assert_eq!(got.max_bucket, old.max_bucket);
                    assert_eq!(got.fill_edges, old.fill_edges);
                    assert_eq!(got.candidate_evaluations, old.candidate_evaluations);
                }
            }
        }
    }

    #[test]
    fn source_scope_validation_and_identical_ranking() {
        let clauses = vec![vec![1, 2], vec![1, 3], vec![1, 4], vec![2, 3]];
        let graph = FastGraph::build(&clauses, &[4, 2, 1, 3]).unwrap();
        assert_eq!(graph.ranked(4), vec![1, 2, 3, 4]);
        assert!(FastGraph::build(&clauses, &[1, 1, 2, 3]).is_none());
        assert!(FastGraph::build(&clauses, &[1, 2, 3]).is_none());
        let bad = graph.plan_deleted(1, 1, 11);
        assert!(bad.capped);
    }
}
