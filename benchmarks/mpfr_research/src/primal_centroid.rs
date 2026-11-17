//! Research-only, bounded primal-graph bag-centroid variable scores.
//!
//! The GPMC FlowCutter decomposition uses a centroid's bag distance to
//! prioritize decision variables. This is a DISTINCT, inexpensive
//! min-degree triangulation surrogate on the unchanged source Weighted
//! CNF; it is NOT FlowCutter and does not claim identical tree width.
//! Scores only change Shannon pivot order: BOTH weighted branches and
//! every original satisfying assignment remain explicitly counted.
//! Resource/width caps return an all-zero Frequency-fallback profile.
use crate::Problem;
use std::collections::VecDeque;

const MAX_SOURCE_VARS: usize = 4096;
const MAX_ACTIVE: usize = 256;
const MAX_CLAUSES: usize = 3000;
const MAX_CLAUSE_ARITY: usize = 32;
const MAX_EDGES: usize = 20000;
const MAX_FILL: usize = 20000;
const NONE: usize = usize::MAX;

#[derive(Clone, Debug, Default)]
pub(crate) struct Profile {
    pub(crate) scores: Vec<u16>,
    pub(crate) eligible: bool,
    pub(crate) capped: bool,
    pub(crate) vertices: usize,
    pub(crate) primal_edges: usize,
    pub(crate) width: usize,
    pub(crate) fill_edges: usize,
    pub(crate) max_distance: usize,
}

fn fail(mut result: Profile) -> Profile {
    result.capped = true;
    result.scores.fill(0);
    result.eligible = false;
    result
}

pub(crate) fn build(problem: &Problem, scope: &[usize]) -> Profile {
    let mut result = Profile {
        scores: vec![0_u16; problem.n + 1],
        vertices: scope.len(),
        ..Profile::default()
    };
    let n = scope.len();
    if problem.n > MAX_SOURCE_VARS
        || !(8..=MAX_ACTIVE).contains(&n)
        || problem.clauses.len() > MAX_CLAUSES
    {
        return fail(result);
    }
    let mut indices = vec![NONE; problem.n + 1];
    for (local, &variable) in scope.iter().enumerate() {
        if variable == 0 || variable > problem.n || indices[variable] != NONE {
            return fail(result);
        }
        indices[variable] = local;
    }
    // Graph vertices are surviving weighted CNF variable IDs. A clause
    // makes all its variables a clique in the primal graph, exactly
    // as in structural treewidth computations; no QASM metadata used.
    let mut adjacency = vec![vec![false; n]; n];
    let mut edges = 0_usize;
    for clause in &problem.clauses {
        let mut local = Vec::with_capacity(clause.len());
        for &literal in clause {
            let variable = literal.unsigned_abs() as usize;
            if variable == 0 || variable > problem.n || indices[variable] == NONE {
                return fail(result);
            }
            local.push(indices[variable]);
        }
        local.sort_unstable();
        local.dedup();
        if local.len() > MAX_CLAUSE_ARITY {
            return fail(result);
        }
        for (at, &a) in local.iter().enumerate() {
            for &b in &local[at + 1..] {
                if !adjacency[a][b] {
                    edges += 1;
                    if edges > MAX_EDGES {
                        return fail(result);
                    }
                    adjacency[a][b] = true;
                    adjacency[b][a] = true;
                }
            }
        }
    }
    result.primal_edges = edges;
    if edges == 0 {
        return result;
    }

    // Triangulate using bounded minimum-current-degree elimination.
    // The surviving neighbors form a bag. A parent edge to the
    // earliest later neighbor constructs an elimination bag forest.
    let mut alive = vec![true; n];
    let mut order = Vec::with_capacity(n);
    let mut bags: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut fill_edges = 0_usize;
    for _ in 0..n {
        let next = (0..n).filter(|&v| alive[v]).min_by_key(|&v| {
            let degree = (0..n).filter(|&u| alive[u] && adjacency[v][u]).count();
            (degree, scope[v])
        }).expect("nonempty elimination scope");
        let later: Vec<usize> = (0..n)
            .filter(|&v| alive[v] && adjacency[next][v])
            .collect();
        result.width = result.width.max(later.len());
        // A dense/poor decomposition is not a license to change the
        // original weighted solver search or create unbounded fill.
        if later.len() * 2 > n {
            return fail(result);
        }
        for (at, &v) in later.iter().enumerate() {
            for &u in &later[at + 1..] {
                if !adjacency[u][v] {
                    fill_edges += 1;
                    if fill_edges > MAX_FILL {
                        return fail(result);
                    }
                    adjacency[u][v] = true;
                    adjacency[v][u] = true;
                }
            }
        }
        bags[next] = later;
        alive[next] = false;
        order.push(next);
    }
    result.fill_edges = fill_edges;
    let mut position = vec![NONE; n];
    for (step, &var) in order.iter().enumerate() {
        position[var] = step;
    }
    let mut tree = vec![Vec::new(); n];
    for &var in &order {
        if let Some(&parent) = bags[var].iter().min_by_key(|&&u| position[u]) {
            // A parent always has later elimination rank, so the
            // resulting graph is a forest and cannot contain cycles.
            debug_assert!(position[parent] > position[var]);
            tree[var].push(parent);
            tree[parent].push(var);
        }
    }

    // Compute one centroid per tree component by minimizing its
    // largest remaining subtree. This is a structural bag-centroid
    // surrogate, not the exact FlowCutter tree decomposition.
    let mut visited = vec![false; n];
    let mut parent = vec![NONE; n];
    let mut subsize = vec![1_usize; n];
    let mut dist = vec![NONE; n];
    for start in 0..n {
        if visited[start] {
            continue;
        }
        let mut traversal = vec![start];
        visited[start] = true;
        parent[start] = start;
        let mut at = 0;
        while at < traversal.len() {
            let var = traversal[at];
            for &neighbor in &tree[var] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    parent[neighbor] = var;
                    traversal.push(neighbor);
                }
            }
            at += 1;
        }
        let size = traversal.len();
        for &var in traversal.iter().rev() {
            if var != start {
                subsize[parent[var]] += subsize[var];
            }
        }
        let center = *traversal.iter().min_by_key(|&&var| {
            let mut largest = size - subsize[var];
            for &neighbor in &tree[var] {
                if parent[neighbor] == var {
                    largest = largest.max(subsize[neighbor]);
                }
            }
            (largest, scope[var])
        }).expect("connected component nonempty");

        let mut queue = VecDeque::new();
        queue.push_back(center);
        dist[center] = 0;
        let mut max_distance = 0_usize;
        while let Some(var) = queue.pop_front() {
            for &neighbor in &tree[var] {
                if dist[neighbor] == NONE {
                    dist[neighbor] = dist[var] + 1;
                    max_distance = max_distance.max(dist[neighbor]);
                    queue.push_back(neighbor);
                }
            }
        }
        result.max_distance = result.max_distance.max(max_distance);
        if max_distance > 0 {
            for &var in &traversal {
                let score = ((max_distance - dist[var]) * 100 / max_distance) as u16;
                result.scores[scope[var]] = score;
            }
        }
    }
    result.eligible = result.scores.iter().any(|&s| s > 0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, trail_engine::TrailEngine};

    #[test]
    fn sparse_primal_path_has_a_center_priority() {
        let cnf = "p cnf 9 8\nc t wmc\n1 2 0\n2 3 0\n3 4 0\n4 5 0\n5 6 0\n6 7 0\n7 8 0\n8 9 0\n";
        let p = parse_emitted(cnf, 100).unwrap();
        let s = build(&p, &(1..=p.n).collect::<Vec<_>>());
        assert!(s.eligible);
        assert_eq!(s.vertices, 9);
        assert_eq!(s.width, 1);
        assert!(s.scores.iter().any(|&x| x > 0));
        assert!(!s.capped);
    }

    #[test]
    fn dense_primal_is_fail_closed_frequency_only() {
        let mut cnf = String::from("p cnf 12 1\nc t wmc\n");
        for var in 1..=12 { cnf.push_str(&format!("{var} ")); }
        cnf.push_str("0\n");
        let p = parse_emitted(&cnf, 100).unwrap();
        let s = build(&p, &(1..=p.n).collect::<Vec<_>>());
        assert!(s.capped);
        assert!(!s.eligible);
        assert!(s.scores.iter().all(|&x| x == 0));
    }

    #[test]
    fn root_and_full_score_choose_both_weighted_branches_without_model_loss() {
        let cnf = "p cnf 9 10\nc t wmc\n\
c p weight 1 0.5 0.25 0\n\
c p weight -1 -0.25 0.5 0\n\
1 2 0\n2 3 0\n3 4 0\n4 5 0\n5 6 0\n6 7 0\n7 8 0\n8 9 0\n1 -9 0\n-3 7 0\n";
        for bits in [100, 333] {
            let p = parse_emitted(cnf, bits).unwrap();
            let profile = build(&p, &(1..=p.n).collect::<Vec<_>>());
            assert!(profile.eligible);
            for capacity in [0_usize, 1, 128] {
                let mut baseline = TrailEngine::new(&p, bits, capacity, 100_000);
                baseline.use_parent_ids = true;
                baseline.use_compact_residual = true;
                baseline.use_prehashed_cache = true;
                baseline.use_fused_compact = true;
                baseline.use_weight_class_cache = true;
                baseline.use_inline_weight_relabel = true;
                let expected = baseline.count((1..=p.n).collect(), 0).unwrap();
                for root_only in [true, false] {
                    let mut candidate = TrailEngine::new(&p, bits, capacity, 100_000);
                    candidate.use_parent_ids = true;
                    candidate.use_compact_residual = true;
                    candidate.use_prehashed_cache = true;
                    candidate.use_fused_compact = true;
                    candidate.use_weight_class_cache = true;
                    candidate.use_inline_weight_relabel = true;
                    candidate.primal_scores = Some(profile.scores.clone());
                    candidate.primal_root_only = root_only;
                    let observed = candidate.count((1..=p.n).collect(), 0).unwrap();
                    assert_eq!(observed.sat, expected.sat);
                    assert_eq!(observed.count.re, expected.count.re);
                    assert_eq!(observed.count.im, expected.count.im);
                    assert!(candidate.assigned.iter().all(|&assignment| assignment == 0));
                }
            }
        }
    }
}
