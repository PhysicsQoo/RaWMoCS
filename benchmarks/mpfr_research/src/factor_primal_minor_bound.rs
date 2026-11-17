//! Research-only primal graph minor-min-width certificate.
//! A graph minor cannot have larger treewidth than its parent.
//! At each step, minimum degree is a lower bound on treewidth;
//! contracting an edge preserves the minor relationship. The maximum
//! such degree over the contraction sequence is a valid LOWER bound
//! on the source primal graph treewidth, not an exact treewidth result.
use crate::Clause;

#[derive(Default, Clone, Debug)]
pub(crate) struct MinorBound {
    pub(crate) treewidth_lower: usize,
    pub(crate) contractions: usize,
    pub(crate) capped: bool,
}

/// Returns a certified lower bound on treewidth; source clause scopes
/// are read-only, original signed-complex weights are not touched.
/// This is polynomial graph-only research, not factor multiplication.
pub(crate) fn minor_min_width(clauses: &[Clause], scope: &[usize]) -> MinorBound {
    let mut out = MinorBound::default();
    if scope.is_empty() || scope.len() > 320 {
        out.capped = true;
        return out;
    }
    let mut vars = scope.to_vec();
    vars.sort_unstable();
    if vars[0] == 0 || vars.windows(2).any(|w| w[0] == w[1]) {
        out.capped = true;
        return out;
    }
    let n = vars.len();
    let mut pos = vec![usize::MAX; vars[n - 1] + 1];
    for (i, &v) in vars.iter().enumerate() { pos[v] = i; }
    let mut edges = vec![vec![false; n]; n];
    for clause in clauses {
        let mut members = Vec::new();
        for &lit in clause {
            let v = lit.unsigned_abs() as usize;
            if v >= pos.len() || pos[v] == usize::MAX {
                out.capped = true;
                return out;
            }
            members.push(pos[v]);
        }
        members.sort_unstable();
        members.dedup();
        for (k, &i) in members.iter().enumerate() {
            for &j in &members[k+1..] {
                edges[i][j] = true;
                edges[j][i] = true;
            }
        }
    }
    let mut active = vec![true; n];
    for _ in 0..n {
        let (v, degree) = (0..n).filter(|&i| active[i])
            .map(|i| (i, (0..n).filter(|&j| active[j] && edges[i][j]).count()))
            .min_by_key(|&(i, degree)| (degree, vars[i]))
            .expect("active vertex exists");
        out.treewidth_lower = out.treewidth_lower.max(degree);
        if degree > 0 {
            // Contract v into an adjacent neighbor with minimum degree.
            let u = (0..n).filter(|&i| active[i] && edges[v][i])
                .min_by_key(|&i| ((0..n).filter(|&j| active[j] && edges[i][j]).count(), vars[i]))
                .unwrap();
            for w in 0..n {
                if active[w] && w != u && w != v && edges[v][w] {
                    edges[u][w] = true;
                    edges[w][u] = true;
                }
            }
            out.contractions += 1;
        }
        active[v] = false;
        for i in 0..n { edges[v][i] = false; edges[i][v] = false; }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clique_twelve_proves_table_width_twelve_required() {
        let vars: Vec<usize> = (1..=12).collect();
        let mut clauses = Vec::new();
        for i in 1..=12 { for j in i+1..=12 { clauses.push(vec![i, j]); } }
        let b = minor_min_width(&clauses, &vars);
        assert!(!b.capped);
        assert_eq!(b.treewidth_lower, 11);
        assert_eq!(b.contractions, 11);
    }
    #[test]
    fn four_cycle_has_treewidth_two() {
        let b = minor_min_width(&[vec![1,2],vec![2,3],vec![3,4],vec![4,1]], &[4,3,2,1]);
        assert!(!b.capped);
        assert_eq!(b.treewidth_lower, 2);
    }
    #[test]
    fn disconnected_and_isolated_nodes_do_not_create_fill() {
        let b = minor_min_width(&[vec![1,2],vec![3,4]], &[1,2,3,4,5]);
        assert!(!b.capped);
        assert_eq!(b.treewidth_lower, 1);
        let isolated: Vec<usize> = (1..=257).collect();
        assert_eq!(minor_min_width(&[], &isolated).treewidth_lower, 0);
    }
    #[test]
    fn reject_invalid_scope_or_clause_outside_scope() {
        assert!(minor_min_width(&[vec![1,3]], &[1,2]).capped);
        assert!(minor_min_width(&[], &[1,1]).capped);
    }
}
