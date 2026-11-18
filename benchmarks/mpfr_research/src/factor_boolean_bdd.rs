//! Research-only Boolean ROBDD compilation followed by exact signed-complex
//! weighted evaluation, on the unchanged source Weighted CNF.
//!
//! Unlike an algebraic ADD, this diagram shares only Boolean cofactors;
//! high-entropy signed weights are applied after Boolean reduction. Every
//! source variable's positive/negative unary weight is counted exactly once,
//! including variables suppressed by BDD reduction (free/skipped levels).
//! Boolean satisfiability never depends on a possibly cancelled complex WMC.
use crate::{factor_minfill_order, C, Clause, Outcome, Problem};
use std::collections::HashMap;

const MAX_SOURCE_VARS: usize = 4096;
const MAX_SCOPE: usize = 320;
const MAX_CLAUSES: usize = 3000;
const MAX_CLAUSE_WIDTH: usize = 12;
const MAX_NODES: usize = 50_000;
const MAX_APPLY_CALLS: usize = 1_000_000;
const MAX_CACHE_ENTRIES: usize = 160_000;
const MAX_EVAL_CALLS: usize = 180_000;

#[derive(Clone, Debug, Default)]
pub(crate) struct BddStats {
    pub(crate) eligible: bool,
    pub(crate) capped: bool,
    pub(crate) reason: &'static str,
    pub(crate) reverse_minfill: bool,
    pub(crate) variables: usize,
    pub(crate) clauses: usize,
    pub(crate) nodes: usize,
    pub(crate) apply_calls: usize,
    pub(crate) apply_hits: usize,
    pub(crate) eval_calls: usize,
    pub(crate) eval_hits: usize,
    pub(crate) minfill_bucket: usize,
}

#[derive(Clone, Copy)]
enum Node {
    False,
    True,
    Branch { rank: usize, low: usize, high: usize },
}

struct BooleanBdd {
    nodes: Vec<Node>,
    unique: HashMap<(usize, usize, usize), usize>,
    and_cache: HashMap<(usize, usize), usize>,
    eval_cache: HashMap<usize, Outcome>,
    stats: BddStats,
}

impl BooleanBdd {
    fn new(reverse_minfill: bool) -> Self {
        Self {
            nodes: vec![Node::False, Node::True],
            unique: HashMap::new(),
            and_cache: HashMap::new(),
            eval_cache: HashMap::new(),
            stats: BddStats { reverse_minfill, ..BddStats::default() },
        }
    }

    fn reject<T>(&mut self, reason: &'static str) -> Option<T> {
        self.stats.capped = true;
        if self.stats.reason.is_empty() {
            self.stats.reason = reason;
        }
        None
    }

    fn rank_of(&self, node: usize, total: usize) -> usize {
        match self.nodes[node] {
            Node::Branch { rank, .. } => rank,
            _ => total,
        }
    }

    fn branch(&mut self, rank: usize, low: usize, high: usize) -> Option<usize> {
        if low == high { return Some(low); }
        let key = (rank, low, high);
        if let Some(&existing) = self.unique.get(&key) {
            return Some(existing);
        }
        if self.nodes.len() >= MAX_NODES {
            return self.reject("bdd_nodes");
        }
        let id = self.nodes.len();
        self.nodes.push(Node::Branch { rank, low, high });
        self.unique.insert(key, id);
        Some(id)
    }

    fn and(&mut self, a: usize, b: usize) -> Option<usize> {
        self.stats.apply_calls += 1;
        if self.stats.apply_calls > MAX_APPLY_CALLS {
            return self.reject("bdd_apply_calls");
        }
        if a == 0 || b == 0 { return Some(0); }
        if a == 1 { return Some(b); }
        if b == 1 || a == b { return Some(a); }
        let key = (a.min(b), a.max(b));
        if let Some(&hit) = self.and_cache.get(&key) {
            self.stats.apply_hits += 1;
            return Some(hit);
        }
        let (ar, al, ah) = match self.nodes[a] {
            Node::Branch { rank, low, high } => (rank, low, high),
            _ => unreachable!(),
        };
        let (br, bl, bh) = match self.nodes[b] {
            Node::Branch { rank, low, high } => (rank, low, high),
            _ => unreachable!(),
        };
        let rank = ar.min(br);
        let (a0, a1) = if ar == rank { (al, ah) } else { (a, a) };
        let (b0, b1) = if br == rank { (bl, bh) } else { (b, b) };
        let low = self.and(a0, b0)?;
        let high = self.and(a1, b1)?;
        let combined = self.branch(rank, low, high)?;
        if self.and_cache.len() >= MAX_CACHE_ENTRIES {
            return self.reject("bdd_and_cache");
        }
        self.and_cache.insert(key, combined);
        Some(combined)
    }

    fn clause(&mut self, clause: &Clause, positions: &[usize]) -> Option<usize> {
        if clause.len() > MAX_CLAUSE_WIDTH {
            return self.reject("bdd_clause_width");
        }
        let mut signs: HashMap<usize, u8> = HashMap::new();
        for &lit in clause {
            let var = lit.unsigned_abs() as usize;
            if var == 0 || var >= positions.len() || positions[var] == usize::MAX {
                return self.reject("bdd_clause_scope");
            }
            let r = positions[var];
            let entry = signs.entry(r).or_insert(0);
            *entry |= if lit > 0 { 1 } else { 2 };
            if *entry == 3 {
                // A tautological clause contributes the true Boolean factor.
                return Some(1);
            }
        }
        let mut vars: Vec<_> = signs.into_iter().collect();
        vars.sort_unstable_by_key(|&(r, _)| r);
        let mut root = 0;
        for (r, sign) in vars.into_iter().rev() {
            root = if sign == 1 {
                self.branch(r, root, 1)?
            } else {
                self.branch(r, 1, root)?
            };
        }
        Some(root)
    }

    fn gap(&self, start: usize, end: usize, free: &[C], bits: u32) -> C {
        let mut value = C::one(bits);
        for v in &free[start..end] {
            value = value.mul(v, bits);
        }
        value
    }

    fn weighted_node(
        &mut self, node: usize, variables: &[usize], p: &Problem, free: &[C],
        bits: u32,
    ) -> Option<Outcome> {
        if node == 0 { return Some(Outcome::unsat(bits)); }
        if node == 1 {
            return Some(Outcome { sat: true, count: C::one(bits) });
        }
        self.stats.eval_calls += 1;
        if self.stats.eval_calls > MAX_EVAL_CALLS {
            return self.reject("bdd_eval_calls");
        }
        if let Some(hit) = self.eval_cache.get(&node) {
            self.stats.eval_hits += 1;
            return Some(hit.clone());
        }
        let Node::Branch { rank, low, high } = self.nodes[node] else {
            unreachable!()
        };
        let n = variables.len();
        let lo = self.weighted_node(low, variables, p, free, bits)?;
        let hi = self.weighted_node(high, variables, p, free, bits)?;
        let var = variables[rank];
        let weights = &p.weights[var - 1];

        let low_count = if lo.sat {
            let child_rank = self.rank_of(low, n);
            let skipped = self.gap(rank + 1, child_rank, free, bits);
            weights.negative.mul(&skipped.mul(&lo.count, bits), bits)
        } else {
            C::zero(bits)
        };
        let high_count = if hi.sat {
            let child_rank = self.rank_of(high, n);
            let skipped = self.gap(rank + 1, child_rank, free, bits);
            weights.positive.mul(&skipped.mul(&hi.count, bits), bits)
        } else {
            C::zero(bits)
        };
        let result = Outcome {
            sat: lo.sat || hi.sat,
            count: low_count.add(&high_count, bits),
        };
        if self.eval_cache.len() >= MAX_CACHE_ENTRIES {
            return self.reject("bdd_eval_cache");
        }
        self.eval_cache.insert(node, result.clone());
        Some(result)
    }

    fn solve(
        &mut self, p: &Problem, scope: &[usize], bits: u32,
    ) -> Option<Outcome> {
        self.stats.variables = scope.len();
        self.stats.clauses = p.clauses.len();
        if p.n == 0 || p.n > MAX_SOURCE_VARS
            || scope.is_empty() || scope.len() > MAX_SCOPE
            || p.clauses.len() > MAX_CLAUSES || p.weights.len() < p.n {
            return self.reject("bdd_source_envelope");
        }
        let mut variables = scope.to_vec();
        variables.sort_unstable();
        if variables.iter().any(|&v| v == 0 || v > p.n)
            || variables.windows(2).any(|pair| pair[0] == pair[1]) {
            return self.reject("bdd_invalid_scope");
        }
        if self.stats.reverse_minfill {
            let plan = factor_minfill_order::plan(&p.clauses, &variables, MAX_SCOPE);
            if plan.capped || plan.order.len() != variables.len() {
                return self.reject("bdd_minfill_plan");
            }
            self.stats.minfill_bucket = plan.max_bucket;
            variables = plan.order.into_iter().rev().collect();
        }
        let mut positions = vec![usize::MAX; p.n + 1];
        for (rank, &v) in variables.iter().enumerate() {
            positions[v] = rank;
        }
        // All source Boolean clauses, no QASM/intermediate rewrite or
        // accidental loss of weighted free or skipped source variables.
        self.stats.eligible = true;
        let mut root = 1;
        for clause in &p.clauses {
            let factor = self.clause(clause, &positions)?;
            root = self.and(root, factor)?;
            if root == 0 { break; }
        }
        self.stats.nodes = self.nodes.len();

        let mut free = Vec::with_capacity(variables.len());
        for &v in &variables {
            free.push(p.weights[v - 1].negative.add(&p.weights[v - 1].positive, bits));
        }
        let value = if root == 0 {
            Outcome::unsat(bits)
        } else if root == 1 {
            Outcome { sat: true, count: self.gap(0, variables.len(), &free, bits) }
        } else {
            let count = self.weighted_node(root, &variables, p, &free, bits)?;
            let prefix = self.gap(0, self.rank_of(root, variables.len()), &free, bits);
            Outcome { sat: count.sat, count: prefix.mul(&count.count, bits) }
        };
        self.stats.nodes = self.nodes.len();
        Some(value)
    }
}

pub(crate) fn count(
    p: &Problem, scope: &[usize], bits: u32, reverse_minfill: bool,
) -> (Option<Outcome>, BddStats) {
    let mut bdd = BooleanBdd::new(reverse_minfill);
    let result = bdd.solve(p, scope, bits);
    bdd.stats.nodes = bdd.nodes.len();
    (result, bdd.stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, trail_engine::TrailEngine};

    fn brute(p: &Problem, bits: u32) -> Outcome {
        assert!(p.n <= 10);
        let mut sat = false;
        let mut total = C::zero(bits);
        for mask in 0..(1_usize << p.n) {
            if p.clauses.iter().any(|clause| {
                clause.iter().all(|&lit| {
                    let bit = ((mask >> (lit.unsigned_abs() as usize - 1)) & 1) != 0;
                    bit != (lit > 0)
                })
            }) {
                continue;
            }
            sat = true;
            let mut product = C::one(bits);
            for (i, weights) in p.weights.iter().enumerate().take(p.n) {
                product = product.mul(
                    if (mask >> i) & 1 == 1 {
                        &weights.positive
                    } else {
                        &weights.negative
                    }, bits,
                );
            }
            total = total.add(&product, bits);
        }
        Outcome { sat, count: total }
    }

    fn check(raw: &str) {
        for bits in [100, 333] {
            let p = parse_emitted(raw, bits).unwrap();
            let scope: Vec<usize> = (1..=p.n).collect();
            let exact = brute(&p, bits);
            for mode in [false, true] {
                let (result, stats) = count(&p, &scope, bits, mode);
                assert!(result.is_some() && !stats.capped, "{stats:?}");
                let result = result.unwrap();
                assert_eq!(result.sat, exact.sat);
                assert_eq!(result.count.re, exact.count.re);
                assert_eq!(result.count.im, exact.count.im);
                let mut trail = TrailEngine::new(&p, bits, 128, 100_000);
                let expected = trail.count(scope.clone(), 0).unwrap();
                assert_eq!(result.sat, expected.sat);
                assert_eq!(result.count.re, expected.count.re);
                assert_eq!(result.count.im, expected.count.im);
            }
        }
    }

    #[test]
    fn signed_complex_weighted_connected_cnfs() {
        check("p cnf 4 3\nc t wmc\n\
c p weight 1 -0.5 0.25 0\nc p weight -1 0.75 -0.5 0\n\
c p weight 2 0.25 0.5 0\nc p weight -2 -0.5 0.25 0\n\
1 -2 0\n2 3 0\n-3 4 0\n");
    }

    #[test]
    fn true_sat_with_zero_wmc_and_true_unsat() {
        check("p cnf 2 1\nc t wmc\n\
c p weight 1 1 0 0\nc p weight -1 -1 0 0\n2 0\n");
        check("p cnf 2 2\nc t wmc\n1 0\n-1 0\n");
        check("p cnf 2 1\nc t wmc\n0\n");
    }

    #[test]
    fn tautology_duplicate_literals_free_and_disconnected() {
        check("p cnf 6 5\nc t wmc\n\
c p weight 6 -0.5 0.5 0\nc p weight -6 0.25 -0.25 0\n\
1 1 2 0\n-1 -1 3 0\n4 -5 0\n-4 6 0\n2 -2 0\n");
    }

    #[test]
    fn boolean_shared_subproblems_not_complex_terminals() {
        let raw = "p cnf 8 8\nc t wmc\n\
c p weight 1 0.125 0.25 0\nc p weight -1 0.375 0 0\n\
c p weight 5 -0.25 0.125 0\nc p weight -5 0.75 -0.25 0\n\
1 2 0\n-1 3 0\n2 -4 0\n-2 5 0\n-3 6 0\n\
4 -7 0\n-5 8 0\n-6 -8 0\n";
        check(raw);
        let p = parse_emitted(raw, 100).unwrap();
        let (_, stats) = count(&p, &(1..=8).collect::<Vec<_>>(), 100, true);
        assert!(stats.nodes < 1000, "{stats:?}");
    }

    #[test]
    fn invalid_envelope_falls_back_without_partial_answer() {
        let p = parse_emitted("p cnf 2 1\nc t wmc\n1 -2 0\n", 100).unwrap();
        let (answer, stats) = count(&p, &[1, 1], 100, false);
        assert!(answer.is_none() && stats.capped);
        assert_eq!(stats.reason, "bdd_invalid_scope");
        let mut trail = TrailEngine::new(&p, 100, 128, 100_000);
        let fallback = trail.count(vec![1, 2], 0).unwrap();
        let exact = brute(&p, 100);
        assert_eq!(fallback.sat, exact.sat);
        assert_eq!(fallback.count.re, exact.count.re);
        assert_eq!(fallback.count.im, exact.count.im);
    }
}
