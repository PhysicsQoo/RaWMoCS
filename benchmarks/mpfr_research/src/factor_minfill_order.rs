//! Independent bounded primal-graph min-fill elimination order for the
//! exact signed-complex factor solver. Graph structure only: it never
//! transforms CNF, weight ownership, or Boolean/Sum-Product semantics.
use crate::Clause;

#[derive(Default, Clone, Debug)]
pub(crate) struct FillPlan {
    pub(crate) order: Vec<usize>,
    pub(crate) max_bucket: usize,
    pub(crate) fill_edges: usize,
    pub(crate) candidate_evaluations: u64,
    pub(crate) capped: bool,
}

/// Min-fill with an explicit feasible-width-first admission guard.
/// No exponential factor tables are built here. Each source-CNF clause
/// is a factor scope, and eliminated variables create fill cliques.
pub(crate) fn plan(
    clauses: &[Clause], scope: &[usize], max_bucket: usize,
) -> FillPlan {
    let mut result = FillPlan::default();
    if scope.is_empty() || scope.len() > 320 || max_bucket == 0 {
        result.capped = true;
        return result;
    }
    let mut vars = scope.to_vec();
    vars.sort_unstable();
    if vars.windows(2).any(|w| w[0] == w[1]) {
        result.capped = true;
        return result;
    }
    let n = vars.len();
    let max_id = *vars.last().unwrap();
    let mut position = vec![usize::MAX; max_id + 1];
    for (i, &v) in vars.iter().enumerate() {
        if v == 0 {
            result.capped = true;
            return result;
        }
        position[v] = i;
    }
    let mut adj = vec![vec![false; n]; n];
    for clause in clauses {
        let mut indices = Vec::with_capacity(clause.len());
        for &lit in clause {
            let var = lit.unsigned_abs() as usize;
            if var > max_id || position[var] == usize::MAX {
                result.capped = true;
                return result;
            }
            indices.push(position[var]);
        }
        indices.sort_unstable();
        indices.dedup();
        for (k, &a) in indices.iter().enumerate() {
            for &b in &indices[k + 1..] {
                adj[a][b] = true;
                adj[b][a] = true;
            }
        }
    }
    let mut active = vec![true; n];
    for _ in 0..n {
        let mut best: Option<(bool, usize, usize, usize, Vec<usize>)> = None;
        for (i, &var) in vars.iter().enumerate() {
            if !active[i] {
                continue;
            }
            let neighbors: Vec<usize> = (0..n).filter(|&j| active[j] && adj[i][j]).collect();
            let bucket = neighbors.len() + 1;
            let mut fill = 0;
            for (k, &a) in neighbors.iter().enumerate() {
                for &b in &neighbors[k + 1..] {
                    if !adj[a][b] {
                        fill += 1;
                    }
                }
            }
            result.candidate_evaluations += 1;
            let key = (bucket > max_bucket, fill, bucket, var);
            if best.as_ref().is_none_or(|x| key < (x.0, x.1, x.2, x.3)) {
                best = Some((key.0, key.1, key.2, key.3, neighbors));
            }
        }
        let (over, _, bucket, var, neighbors) = best.expect("nonempty live graph");
        result.max_bucket = result.max_bucket.max(bucket);
        if over {
            result.capped = true;
            return result;
        }
        for (k, &a) in neighbors.iter().enumerate() {
            for &b in &neighbors[k + 1..] {
                if !adj[a][b] {
                    adj[a][b] = true;
                    adj[b][a] = true;
                    result.fill_edges += 1;
                }
            }
        }
        result.order.push(var);
        active[position[var]] = false;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_257_scope_has_no_fill_and_admits_width_one() {
        let scope: Vec<usize> = (1..=257).collect();
        let clauses: Vec<Clause> = scope.iter().map(|&v| vec![v as i32]).collect();
        let profile = plan(&clauses, &scope, 11);
        assert!(!profile.capped);
        assert_eq!(profile.order, scope);
        assert_eq!(profile.max_bucket, 1);
        assert_eq!(profile.fill_edges, 0);
    }

    #[test]
    fn dense_clique_fails_width_without_building_any_factor_table() {
        let scope: Vec<usize> = (1..=12).collect();
        let mut clauses = Vec::new();
        for i in 1..=12 {
            for j in (i + 1)..=12 {
                clauses.push(vec![i, j]);
            }
        }
        let profile = plan(&clauses, &scope, 11);
        assert!(profile.capped);
        assert_eq!(profile.max_bucket, 12);
        assert!(profile.order.is_empty());
    }

    #[test]
    fn min_fill_can_select_a_different_variable_from_min_degree() {
        // Graph edges: (1,2), (1,4), (1,5), (2,3), (2,5), (3,4).
        // Min-degree picks variable 3; min-fill picks variable 5,
        // whose two neighbors are already connected (zero fill).
        let clauses = vec![
            vec![1, 2], vec![1, 4], vec![1, 5],
            vec![2, 3], vec![2, 5], vec![3, 4],
        ];
        let planned = plan(&clauses, &[1, 2, 3, 4, 5], 11);
        assert!(!planned.capped);
        assert_eq!(planned.order.len(), 5);
        assert_eq!(planned.order[0], 5);
        assert_ne!(planned.order[0], 3);
    }

    #[test]
    fn a_four_cycle_is_triangulated_deterministically() {
        let clauses = vec![vec![1, 2], vec![2, 3], vec![3, 4], vec![4, 1]];
        let profile = plan(&clauses, &[4, 2, 3, 1], 11);
        assert_eq!(profile.order.len(), 4);
        assert_eq!(profile.max_bucket, 3);
        assert_eq!(profile.fill_edges, 1);
        assert!(!profile.capped);
    }
}
