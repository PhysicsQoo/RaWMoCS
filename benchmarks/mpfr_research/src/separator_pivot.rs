//! Research-only bounded articulation-point pivot on a residual CNF factor graph.
//! No clauses or weights are rewritten; choosing a pivot only reorders exact
//! Shannon branches. Tarjan lowlink: O(V + clauses + literal incidences).
use crate::Clause;
const MAX_VARS: usize = 256;
const MAX_CLAUSES: usize = 4096;
const MAX_EDGES: usize = 8192;
const NONE: usize = usize::MAX;

#[derive(Default, Clone, Copy)]
pub(crate) struct Probe {
    pub(crate) pivot: Option<usize>,
    pub(crate) candidates: usize,
    pub(crate) scanned: bool,
}

#[allow(clippy::too_many_arguments)]
fn visit(
    at: usize, parent: usize, graph: &[Vec<usize>], var_count: usize,
    seen: &mut [usize], low: &mut [usize], clock: &mut usize, cuts: &mut [usize],
) {
    seen[at] = *clock;
    low[at] = *clock;
    *clock += 1;
    let mut children = 0;
    let mut separated = 0;
    for &neighbor in &graph[at] {
        if seen[neighbor] == NONE {
            children += 1;
            visit(neighbor, at, graph, var_count, seen, low, clock, cuts);
            low[at] = low[at].min(low[neighbor]);
            if low[neighbor] >= seen[at] { separated += 1; }
        } else if neighbor != parent {
            low[at] = low[at].min(seen[neighbor]);
        }
    }
    if at < var_count {
        cuts[at] = if parent == NONE {
            if children >= 2 { children } else { 0 }
        } else if separated > 0 { separated + 1 } else { 0 };
    }
}

/// Returns a variable whose removal disconnects a connected residual
/// variable-clause graph. Bounded or malformed inputs fall back to Frequency.
pub(crate) fn choose(
    active: &[usize], ids: &[usize], clauses: &[Clause],
    assigned: &[i8], satisfied: &[usize], frequency: &[usize],
) -> Probe {
    let mut result = Probe::default();
    let count = active.len();
    if !(3..=MAX_VARS).contains(&count) || ids.len() > MAX_CLAUSES { return result; }
    let mut local = vec![NONE; assigned.len()];
    for (index, &var) in active.iter().enumerate() {
        if var >= assigned.len() || assigned[var] != 0 { return result; }
        local[var] = index;
    }
    let mut graph = vec![Vec::<usize>::new(); count];
    let mut incidences = 0;
    for &id in ids {
        if id >= clauses.len() || id >= satisfied.len() { return result; }
        if satisfied[id] > 0 { continue; }
        let mut neighbors = Vec::new();
        for &lit in &clauses[id] {
            let var = lit.unsigned_abs() as usize;
            if var >= assigned.len() { return result; }
            if assigned[var] == 0 {
                if local[var] == NONE { return result; }
                neighbors.push(local[var]);
            }
        }
        // A unit/empty clause must be handled by propagation, not this graph.
        if neighbors.len() < 2 { return result; }
        incidences += neighbors.len();
        if incidences > MAX_EDGES || graph.len() - count >= MAX_CLAUSES {
            return result;
        }
        let clause_node = graph.len();
        graph.push(Vec::with_capacity(neighbors.len()));
        for v in neighbors {
            graph[v].push(clause_node);
            graph[clause_node].push(v);
        }
    }
    result.scanned = true;
    let mut seen = vec![NONE; graph.len()];
    let mut low = vec![NONE; graph.len()];
    let mut cuts = vec![0; count];
    let mut clock = 0;
    for vertex in 0..graph.len() {
        if seen[vertex] == NONE {
            visit(vertex, NONE, &graph, count, &mut seen, &mut low, &mut clock, &mut cuts);
        }
    }
    let mut best: Option<(usize, usize, usize)> = None;
    for (index, &var) in active.iter().enumerate() {
        if cuts[index] >= 2 {
            result.candidates += 1;
            let score = (cuts[index], frequency[var], var);
            if best.is_none_or(|current| score > current) { best = Some(score); }
        }
    }
    result.pivot = best.map(|(_, _, var)| var);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(n: usize, clauses: Vec<Clause>) -> Probe {
        let mut freq = vec![0; n + 1];
        for clause in &clauses {
            for &lit in clause { freq[lit.unsigned_abs() as usize] += 1; }
        }
        let ids: Vec<_> = (0..clauses.len()).collect();
        choose(&(1..=n).collect::<Vec<_>>(), &ids, &clauses,
               &vec![0; n + 1], &vec![0; ids.len()], &freq)
    }
    #[test]
    fn bridge_between_biconnected_triangles() {
        let probe = check(5, vec![vec![1, 2], vec![1, 3], vec![2, 3],
                                  vec![3, 4], vec![3, 5], vec![4, 5]]);
        assert!(probe.scanned);
        assert_eq!(probe.pivot, Some(3));
        assert_eq!(probe.candidates, 1);
    }
    #[test]
    fn cycle_has_no_articulation() {
        let p = check(4, vec![vec![1, 2], vec![2, 3],
                              vec![3, 4], vec![4, 1]]);
        assert!(p.scanned);
        assert_eq!(p.pivot, None);
    }
    #[test]
    fn bounded_or_out_of_scope_is_fail_closed() {
        let p = check(257, vec![vec![1, 2]]);
        assert!(!p.scanned);
        let clauses = vec![vec![1, 4], vec![2, 3]];
        let p = choose(&[1, 2, 3], &[0, 1], &clauses,
                       &[0; 5], &[0; 2], &[0; 5]);
        assert!(!p.scanned);
    }
}
