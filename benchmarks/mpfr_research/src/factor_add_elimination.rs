//! Research-only bounded ordered ADD factor elimination from original weighted CNF.
//! Each leaf retains Boolean SAT *separately* from the signed complex MPFR WMC.
//! No CNF rewrite or default solver change. Any rejected attempt is discarded.
use crate::{factor_minfill_order, C, Clause, Outcome, Problem};
use std::collections::HashMap;

const MAX_SOURCE_VARS: usize = 4096;
const MAX_SCOPE: usize = 320;
const MAX_CLAUSES: usize = 3000;
const MAX_CLAUSE_WIDTH: usize = 12;

#[derive(Clone, Copy)]
struct Limits {
    nodes: usize,
    terminals: usize,
    apply_calls: usize,
    cache_entries: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self { nodes: 30_000, terminals: 4096, apply_calls: 700_000, cache_entries: 120_000 }
    }
}

#[derive(Default, Clone, Debug)]
pub(crate) struct AddStats {
    pub(crate) eligible: bool,
    pub(crate) capped: bool,
    pub(crate) reason: &'static str,
    pub(crate) nodes: usize,
    pub(crate) terminals: usize,
    pub(crate) apply_calls: usize,
    pub(crate) apply_hits: usize,
    pub(crate) abstractions: usize,
    pub(crate) factors: usize,
    pub(crate) eliminated: usize,
    pub(crate) max_bucket: usize,
}

enum Node {
    Terminal(Outcome),
    Branch { var: usize, low: usize, high: usize },
}
type TerminalKey = (bool, (rug::Integer, i32), (rug::Integer, i32));

struct Factor {
    vars: Vec<usize>,
    root: usize,
}

struct Diagram {
    bits: u32,
    limits: Limits,
    nodes: Vec<Node>,
    terminals: Vec<usize>,
    terminal_index: HashMap<TerminalKey, usize>,
    unique: HashMap<(usize, usize, usize), usize>,
    apply_cache: HashMap<(u8, usize, usize), usize>,
    abstract_cache: HashMap<(usize, usize), usize>,
    stats: AddStats,
}
impl Diagram {
    fn new(bits: u32, limits: Limits) -> Self {
        Self {
            bits, limits, nodes: Vec::new(), terminals: Vec::new(),
            terminal_index: HashMap::new(),
            unique: HashMap::new(), apply_cache: HashMap::new(),
            abstract_cache: HashMap::new(), stats: AddStats::default(),
        }
    }

    fn cap(&mut self, reason: &'static str) -> Option<usize> {
        self.stats.capped = true;
        if self.stats.reason.is_empty() {
            self.stats.reason = reason;
        }
        None
    }

    // Exact MPFR finite representation: integer mantissa * 2^exponent.
    // Hashing cannot silently merge SAT-but-zero with UNSAT or rounded values.
    fn terminal(&mut self, value: Outcome) -> Option<usize> {
        let Some(real) = value.count.re.to_integer_exp() else {
            return self.cap("add_nonfinite_real");
        };
        let Some(imag) = value.count.im.to_integer_exp() else {
            return self.cap("add_nonfinite_imag");
        };
        let key = (value.sat, real, imag);
        if let Some(&id) = self.terminal_index.get(&key) {
            match &self.nodes[id] {
                Node::Terminal(previous) if previous.sat == value.sat
                    && previous.count.re == value.count.re
                    && previous.count.im == value.count.im => return Some(id),
                _ => return self.cap("add_terminal_key_invariant"),
            }
        }
        if self.terminals.len() >= self.limits.terminals {
            return self.cap("add_terminals");
        }
        if self.nodes.len() >= self.limits.nodes {
            return self.cap("add_nodes");
        }
        let id = self.nodes.len();
        self.nodes.push(Node::Terminal(value));
        self.terminals.push(id);
        self.terminal_index.insert(key, id);
        Some(id)
    }

    fn branch(&mut self, var: usize, low: usize, high: usize) -> Option<usize> {
        if low == high { return Some(low); }
        let key = (var, low, high);
        if let Some(&existing) = self.unique.get(&key) {
            return Some(existing);
        }
        if self.nodes.len() >= self.limits.nodes {
            return self.cap("add_nodes");
        }
        let id = self.nodes.len();
        self.nodes.push(Node::Branch { var, low, high });
        self.unique.insert(key, id);
        Some(id)
    }

    fn branch_data(&self, id: usize) -> Option<(usize, usize, usize)> {
        match &self.nodes[id] {
            Node::Terminal(_) => None,
            Node::Branch { var, low, high } => Some((*var, *low, *high)),
        }
    }

    // op=0: Boolean OR + complex addition; op=1: AND + complex product.
    fn apply(&mut self, op: u8, a: usize, b: usize) -> Option<usize> {
        self.stats.apply_calls += 1;
        if self.stats.apply_calls > self.limits.apply_calls {
            return self.cap("add_apply_calls");
        }
        let key = (op, a.min(b), a.max(b));
        if let Some(&id) = self.apply_cache.get(&key) {
            self.stats.apply_hits += 1;
            return Some(id);
        }
        let result = match (&self.nodes[a], &self.nodes[b]) {
            (Node::Terminal(x), Node::Terminal(y)) => {
                let outcome = if op == 0 {
                    Outcome {
                        sat: x.sat || y.sat,
                        count: x.count.add(&y.count, self.bits),
                    }
                } else if x.sat && y.sat {
                    Outcome { sat: true, count: x.count.mul(&y.count, self.bits) }
                } else {
                    Outcome::unsat(self.bits)
                };
                self.terminal(outcome)?
            }
            _ => {
                let first = self.branch_data(a);
                let second = self.branch_data(b);
                let var = match (first, second) {
                    (Some((x, _, _)), Some((y, _, _))) => x.min(y),
                    (Some((x, _, _)), None) | (None, Some((x, _, _))) => x,
                    _ => unreachable!(),
                };
                let (al, ah) = match first {
                    Some((x, l, h)) if x == var => (l, h),
                    _ => (a, a),
                };
                let (bl, bh) = match second {
                    Some((x, l, h)) if x == var => (l, h),
                    _ => (b, b),
                };
                let low = self.apply(op, al, bl)?;
                let high = self.apply(op, ah, bh)?;
                self.branch(var, low, high)?
            }
        };
        if self.apply_cache.len() >= self.limits.cache_entries {
            return self.cap("add_apply_cache");
        }
        self.apply_cache.insert(key, result);
        Some(result)
    }

    // Sum out x. If x was suppressed by ADD reduction, both cofactors
    // are identical but must still be summed: every variable is counted.
    fn abstract_sum(&mut self, root: usize, x: usize) -> Option<usize> {
        self.stats.abstractions += 1;
        if self.stats.abstractions > self.limits.apply_calls {
            return self.cap("add_abstractions");
        }
        if let Some(&cached) = self.abstract_cache.get(&(x, root)) {
            return Some(cached);
        }
        let result = match self.branch_data(root) {
            Some((v, low, high)) if v == x => self.apply(0, low, high)?,
            Some((v, low, high)) if v < x => {
                let lo = self.abstract_sum(low, x)?;
                let hi = self.abstract_sum(high, x)?;
                self.branch(v, lo, hi)?
            }
            _ => self.apply(0, root, root)?,
        };
        if self.abstract_cache.len() >= self.limits.cache_entries {
            return self.cap("add_abstract_cache");
        }
        self.abstract_cache.insert((x, root), result);
        Some(result)
    }

    fn clause(&mut self, clause: &Clause, scope: &[bool], on: usize, off: usize)
        -> Option<Factor> {
        let mut signs: HashMap<usize, (bool, bool)> = HashMap::new();
        for &lit in clause {
            let var = lit.unsigned_abs() as usize;
            if var == 0 || var >= scope.len() || !scope[var] {
                return self.cap("add_clause_scope").map(|_| unreachable!());
            }
            let entry = signs.entry(var).or_insert((false, false));
            if lit > 0 { entry.0 = true; } else { entry.1 = true; }
        }
        if signs.len() > MAX_CLAUSE_WIDTH {
            return self.cap("add_clause_width").map(|_| unreachable!());
        }
        let mut vars: Vec<usize> = signs.keys().copied().collect();
        vars.sort_unstable();
        let mut root = off;
        for &v in vars.iter().rev() {
            let (positive, negative) = signs[&v];
            if positive && negative {
                root = on; // tautology: no weight is discarded.
                continue;
            }
            root = if positive {
                self.branch(v, root, on)?
            } else {
                self.branch(v, on, root)?
            };
        }
        Some(Factor { vars, root })
    }

    fn run(&mut self, p: &Problem, scope: &[usize]) -> Option<Outcome> {
        if p.n == 0 || p.n > MAX_SOURCE_VARS || scope.is_empty()
            || scope.len() > MAX_SCOPE || p.clauses.len() > MAX_CLAUSES
            || p.weights.len() < p.n {
            self.cap("add_source_envelope");
            return None;
        }
        let mut present = vec![false; p.n + 1];
        for &v in scope {
            if v == 0 || v > p.n || present[v] {
                self.cap("add_invalid_scope");
                return None;
            }
            present[v] = true;
        }
        self.stats.eligible = true;
        let off = self.terminal(Outcome::unsat(self.bits))?;
        let on = self.terminal(Outcome { sat: true, count: C::one(self.bits) })?;

        let mut factors = Vec::with_capacity(p.clauses.len() + scope.len());
        for clause in &p.clauses {
            factors.push(self.clause(clause, &present, on, off)?);
        }
        for &v in scope {
            let weights = &p.weights[v - 1];
            let low = self.terminal(Outcome { sat: true, count: weights.negative.clone() })?;
            let high = self.terminal(Outcome { sat: true, count: weights.positive.clone() })?;
            let root = self.branch(v, low, high)?;
            factors.push(Factor { vars: vec![v], root });
        }
        self.stats.factors = factors.len();

        // The min-fill planner is used for ordering only; no dense bucket
        // limit is applied to a reduced ADD representation.
        let plan = factor_minfill_order::plan(&p.clauses, scope, MAX_SCOPE);
        if plan.capped || plan.order.len() != scope.len() {
            self.cap("add_order");
            return None;
        }
        self.stats.max_bucket = plan.max_bucket;
        for var in plan.order {
            let mut untouched = Vec::with_capacity(factors.len());
            let mut incident = Vec::new();
            let mut vars = Vec::new();
            for factor in factors {
                if factor.vars.binary_search(&var).is_ok() {
                    vars.extend_from_slice(&factor.vars);
                    incident.push(factor.root);
                } else {
                    untouched.push(factor);
                }
            }
            if incident.is_empty() {
                self.cap("add_missing_unary");
                return None;
            }
            vars.sort_unstable();
            vars.dedup();
            let mut joined = incident[0];
            for &id in &incident[1..] {
                joined = self.apply(1, joined, id)?;
            }
            let summed = self.abstract_sum(joined, var)?;
            vars.retain(|&v| v != var);
            untouched.push(Factor { vars, root: summed });
            factors = untouched;
            self.stats.eliminated += 1;
        }
        let mut result = on;
        for factor in factors {
            if !factor.vars.is_empty() {
                self.cap("add_uneliminated_scope");
                return None;
            }
            result = self.apply(1, result, factor.root)?;
        }
        self.stats.nodes = self.nodes.len();
        self.stats.terminals = self.terminals.len();
        match &self.nodes[result] {
            Node::Terminal(value) => Some(value.clone()),
            _ => {
                self.cap("add_non_scalar");
                None
            }
        }
    }
}

pub(crate) fn count(p: &Problem, scope: &[usize], bits: u32)
    -> (Option<Outcome>, AddStats) {
    // Explicit research-only controlled budget sweep; no production solver
    // and no dense Factor-VE width/cell limits are modified.
    let mult = match std::env::var("RAWMOCS_ADD_RESEARCH_CAP_MULTIPLIER").ok().as_deref() {
        None | Some("1") => 1,
        Some("2") => 2,
        Some("4") => 4,
        _ => return (None, AddStats {
            capped: true, reason: "add_invalid_budget", ..AddStats::default()
        }),
    };
    let mut limits = Limits::default();
    limits.nodes *= mult;
    limits.terminals *= mult;
    limits.apply_calls *= mult;
    limits.cache_entries *= mult;
    count_with_limits(p, scope, bits, limits)
}

fn count_with_limits(p: &Problem, scope: &[usize], bits: u32, limits: Limits)
    -> (Option<Outcome>, AddStats) {
    let mut diagram = Diagram::new(bits, limits);
    let answer = diagram.run(p, scope);
    diagram.stats.nodes = diagram.nodes.len();
    diagram.stats.terminals = diagram.terminals.len();
    (answer, diagram.stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_emitted, trail_engine::TrailEngine};

    // Exhaustive independent assignment oracle; no ADD operations.
    fn brute(p: &Problem, bits: u32) -> Outcome {
        assert!(p.n <= 10);
        let mut sat = false;
        let mut total = C::zero(bits);
        for mask in 0..(1_usize << p.n) {
            if !p.clauses.iter().all(|clause| clause.iter().any(|&lit| {
                let v = lit.unsigned_abs() as usize - 1;
                ((((mask >> v) & 1) == 1) == (lit > 0))
            })) {
                continue;
            }
            sat = true;
            let mut product = C::one(bits);
            for v in 0..p.n {
                let w = if ((mask >> v) & 1) == 1 {
                    &p.weights[v].positive
                } else {
                    &p.weights[v].negative
                };
                product = product.mul(w, bits);
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
            let (result, stats) = count(&p, &scope, bits);
            assert!(result.is_some() && !stats.capped, "{stats:?}");
            let result = result.unwrap();
            assert_eq!(result.sat, exact.sat);
            assert_eq!(result.count.re, exact.count.re);
            assert_eq!(result.count.im, exact.count.im);
            let mut original = TrailEngine::new(&p, bits, 128, 100_000);
            let trail = original.count(scope, 0).unwrap();
            assert_eq!(result.sat, trail.sat);
            assert_eq!(result.count.re, trail.count.re);
            assert_eq!(result.count.im, trail.count.im);
        }
    }

    #[test]
    fn signed_imaginary_weighted_connected_clauses() {
        check("p cnf 4 4\nc t wmc\n\
c p weight 1 -0.5 0.25 0\nc p weight -1 0.75 -0.5 0\n\
c p weight 2 0.25 0.5 0\nc p weight -2 -0.5 0.25 0\n\
1 -2 0\n2 3 0\n-3 4 0\n-4 1 0\n");
    }
    #[test]
    fn sat_but_zero_is_distinct_from_unsat() {
        check("p cnf 2 1\nc t wmc\n\
c p weight 1 1 0 0\nc p weight -1 -1 0 0\n2 0\n");
        check("p cnf 2 2\nc t wmc\n1 0\n-1 0\n");
        check("p cnf 2 1\nc t wmc\n0\n");
    }
    #[test]
    fn tautologies_duplicate_signs_free_and_disconnected() {
        check("p cnf 6 5\nc t wmc\n\
c p weight 6 -0.5 0.5 0\nc p weight -6 0.25 -0.25 0\n\
1 1 2 0\n-1 -1 3 0\n4 -5 0\n-4 6 0\n2 -2 0\n");
    }

    #[test]
    fn terminal_index_preserves_exact_binary_mpfr_and_sat_channel() {
        for bits in [100, 333] {
            let mut diagram = Diagram::new(bits, Limits::default());
            let a = Outcome { sat: true, count: C::decimal(bits, "0.1", "-0.2").unwrap() };
            let first = diagram.terminal(a.clone()).unwrap();
            assert_eq!(diagram.terminal(a).unwrap(), first);
            let changed = Outcome { sat: true, count: C::decimal(bits, "0.1", "-0.20000001").unwrap() };
            assert_ne!(diagram.terminal(changed).unwrap(), first);
            let canceled = Outcome { sat: true, count: C::zero(bits) };
            let impossible = Outcome::unsat(bits);
            assert_ne!(diagram.terminal(canceled).unwrap(),
                       diagram.terminal(impossible).unwrap());
        }
    }

    #[test]
    fn rejected_diagram_does_not_change_original_fallback_count() {
        let p = parse_emitted(
            "p cnf 4 2\nc t wmc\n1 2 0\n3 -4 0\n", 100).unwrap();
        let scope: Vec<usize> = (1..=p.n).collect();
        let (candidate, stats) = count_with_limits(
            &p, &scope, 100,
            Limits { nodes: 2, ..Limits::default() },
        );
        assert!(candidate.is_none() && stats.capped, "{stats:?}");
        assert_eq!(stats.reason, "add_nodes");
        let mut original = TrailEngine::new(&p, 100, 128, 100_000);
        let fallback = original.count(scope, 0).unwrap();
        let exact = brute(&p, 100);
        assert_eq!(fallback.sat, exact.sat);
        assert_eq!(fallback.count.re, exact.count.re);
        assert_eq!(fallback.count.im, exact.count.im);
    }
}
