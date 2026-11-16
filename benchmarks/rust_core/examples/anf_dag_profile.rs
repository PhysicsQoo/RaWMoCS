//! Research-only canonical ANF DAG (ZDD-like square-free monomial family).
//! f = low XOR (x_variable AND high). NOT a WMC solver or backend.
//! Compiled source is bounded and reductions are exact over GF(2).
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs;
use std::time::Instant;

const MAX_VARIABLES: usize = 56;
const MAX_EQUATIONS: usize = 256;
const MAX_MONOMIALS: usize = 200_000;
const MAX_UNIQUE_NODES: usize = 800_000;
const MAX_MASK_VISITS: usize = 4_000_000;

#[derive(Clone)]
struct Equation {
    output: usize,
    terms: Vec<Vec<usize>>,
}

struct Problem {
    variables: usize,
    phase_terms: usize,
    equations: Vec<Equation>,
}

fn integer(raw: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|_| "invalid ANF integer".into())
}
fn parse_problem(raw: &str) -> Result<Problem, String> {
    if raw.len() > 8_000_000 {
        return Err("ANF_SOURCE_BYTE_LIMIT".into());
    }
    let lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
    let head: Vec<&str> = lines
        .first()
        .ok_or("ANF_SOURCE_EMPTY")?
        .split_whitespace()
        .collect();
    if head.len() != 5 || head[0..2] != ["p", "anf"] {
        return Err("ANF_SOURCE_HEADER".into());
    }
    let variables = integer(head[2])?;
    let equation_count = integer(head[3])?;
    let phase_terms = integer(head[4])?;
    if variables > MAX_VARIABLES || equation_count > MAX_EQUATIONS {
        return Err("ANF_SOURCE_DIMENSION_LIMIT".into());
    }
    let mut equations = Vec::with_capacity(equation_count);
    let mut index = 1usize;
    let mut total_terms = 0;
    for _ in 0..equation_count {
        let row: Vec<&str> = lines
            .get(index)
            .ok_or("ANF_MISSING_EQUATION")?
            .split_whitespace()
            .collect();
        index += 1;
        if row.len() != 3 || row[0] != "o" {
            return Err("ANF_EQUATION_HEADER".into());
        }
        let output = integer(row[1])?;
        let arity = integer(row[2])?;
        total_terms += arity;
        if output >= variables || total_terms > MAX_MONOMIALS {
            return Err("ANF_SOURCE_TERM_LIMIT".into());
        }
        let mut terms = Vec::with_capacity(arity);
        for _ in 0..arity {
            let fields: Vec<&str> = lines
                .get(index)
                .ok_or("ANF_MISSING_MONOMIAL")?
                .split_whitespace()
                .collect();
            index += 1;
            if fields.len() < 2 || fields[0] != "m" {
                return Err("ANF_MONOMIAL_HEADER".into());
            }
            let degree = integer(fields[1])?;
            if degree > variables || degree + 2 != fields.len() {
                return Err("ANF_MONOMIAL_DEGREE".into());
            }
            let term = fields[2..]
                .iter()
                .map(|v| integer(v))
                .collect::<Result<Vec<_>, _>>()?;
            if term.iter().any(|&v| v >= variables) {
                return Err("ANF_VARIABLE_RANGE".into());
            }
            terms.push(term);
        }
        equations.push(Equation { output, terms });
    }
    if index != lines.len() {
        return Err("ANF_TRAILING_ROWS".into());
    }
    Ok(Problem {
        variables,
        phase_terms,
        equations,
    })
}

fn mask(vars: &[usize]) -> u64 {
    vars.iter().fold(0, |acc, &v| acc | (1u64 << v))
}
fn trace_poly(eq: &Equation) -> Vec<u64> {
    let mut set = BTreeSet::new();
    let mut toggle = |m: u64| {
        if !set.insert(m) {
            set.remove(&m);
        }
    };
    toggle(1u64 << eq.output);
    for term in &eq.terms {
        toggle(mask(term));
    }
    set.into_iter().collect()
}
#[derive(Clone, Copy)]
struct Node {
    variable: usize,
    low: u32,
    high: u32,
}
struct Dag {
    nodes: Vec<Node>,
    unique: HashMap<(usize, u32, u32), u32>,
    visits: usize,
    reuse_hits: usize,
}
impl Dag {
    fn new() -> Self {
        Self {
            // 0 = zero; 1 = one. Reserved terminals have no edges.
            nodes: vec![
                Node {
                    variable: 0,
                    low: 0,
                    high: 0,
                },
                Node {
                    variable: 0,
                    low: 0,
                    high: 0,
                },
            ],
            unique: HashMap::new(),
            visits: 0,
            reuse_hits: 0,
        }
    }
    fn compile(&mut self, terms: &[u64]) -> Result<u32, String> {
        self.visits = self
            .visits
            .checked_add(terms.len())
            .ok_or("ANF_WORK_COUNTER_OVERFLOW")?;
        if self.visits > MAX_MASK_VISITS {
            return Err("ANF_WORK_LIMIT".into());
        }
        if terms.is_empty() {
            return Ok(0);
        }
        if terms.len() == 1 && terms[0] == 0 {
            return Ok(1);
        }
        let pivot = terms
            .iter()
            .filter(|&&m| m != 0)
            .map(|m| m.trailing_zeros() as usize)
            .min()
            .ok_or("ANF_INVALID_MONOMIAL_SET")?;
        let bit = 1u64 << pivot;
        let mut lows = Vec::new();
        let mut highs = Vec::new();
        for &m in terms {
            if m & bit == 0 {
                lows.push(m);
            } else {
                highs.push(m ^ bit);
            }
        }
        // Both partitions preserve sortedness because bit-removal has
        // no collisions on the high branch for distinct original masks.
        let low = self.compile(&lows)?;
        let high = self.compile(&highs)?;
        if high == 0 {
            return Ok(low);
        }
        let key = (pivot, low, high);
        if let Some(&id) = self.unique.get(&key) {
            self.reuse_hits += 1;
            return Ok(id);
        }
        if self.nodes.len() >= MAX_UNIQUE_NODES {
            return Err("ANF_DAG_NODE_LIMIT".into());
        }
        let id = u32::try_from(self.nodes.len()).map_err(|_| "ANF_DAG_ID_LIMIT")?;
        self.nodes.push(Node {
            variable: pivot,
            low,
            high,
        });
        self.unique.insert(key, id);
        Ok(id)
    }
    fn evaluate(&self, root: u32, assignment: u64, memo: &mut [Option<bool>]) -> bool {
        match root {
            0 => false,
            1 => true,
            _ => {
                let i = root as usize;
                if let Some(value) = memo[i] {
                    return value;
                }
                let node = self.nodes[i];
                let result = self.evaluate(node.low, assignment, memo)
                    ^ ((assignment & (1u64 << node.variable) != 0)
                        && self.evaluate(node.high, assignment, memo));
                memo[i] = Some(result);
                result
            }
        }
    }
}
fn oracle(eq: &Equation, assignment: u64) -> bool {
    let mut value = (assignment & (1u64 << eq.output)) != 0;
    for term in &eq.terms {
        value ^= term.iter().all(|&v| assignment & (1u64 << v) != 0);
    }
    value
}
struct Profile {
    roots: Vec<u32>,
    input_terms: usize,
    canonical_terms: usize,
    literal_occurrences: usize,
    naive_branch_bound: usize,
    nontrivial_outputs: usize,
    max_monomials_per_output: usize,
    max_degree: usize,
    shared_nodes: usize,
    shared_edges: usize,
    max_fanout: usize,
    cross_output_shared_nodes: usize,
    node_count: usize,
    reuse_hits: usize,
    visits: usize,
}
fn profile(problem: &Problem) -> Result<(Dag, Profile), String> {
    let mut dag = Dag::new();
    let mut roots = Vec::with_capacity(problem.equations.len());
    let mut input_terms = 0;
    let mut canonical_terms = 0;
    let mut literal_occurrences = 0;
    let mut max_monomials_per_output = 0;
    let mut max_degree = 0;
    let mut nontrivial_outputs = 0;
    for eq in &problem.equations {
        input_terms += eq.terms.len();
        let terms = trace_poly(eq);
        canonical_terms += terms.len();
        max_monomials_per_output = max_monomials_per_output.max(terms.len());
        literal_occurrences += terms.iter().map(|m| m.count_ones() as usize).sum::<usize>();
        max_degree = max_degree.max(
            terms
                .iter()
                .map(|m| m.count_ones() as usize)
                .max()
                .unwrap_or(0),
        );
        let root = dag.compile(&terms)?;
        nontrivial_outputs += usize::from(root != 0);
        roots.push(root);
    }
    // An explicitly defined non-shared upper bound, not a count of nodes
    // actually allocated by a conventional trie compiler.
    let naive_branch_bound = literal_occurrences + canonical_terms;
    let mut indegree = vec![0usize; dag.nodes.len()];
    for &root in &roots {
        indegree[root as usize] += 1;
    }
    for node in dag.nodes.iter().skip(2) {
        indegree[node.low as usize] += 1;
        indegree[node.high as usize] += 1;
    }
    let shared_nodes = indegree.iter().skip(2).filter(|&&n| n > 1).count();
    let shared_edges = indegree.iter().skip(2).map(|&n| n.saturating_sub(1)).sum();
    let max_fanout = indegree.iter().skip(2).copied().max().unwrap_or(0);
    // Reachability per output, using a per-output visited set; the same
    // interned node reachable from >=2 output roots is cross-output reuse.
    let mut output_references = vec![0usize; dag.nodes.len()];
    for &root in &roots {
        let mut seen = vec![false; dag.nodes.len()];
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if id < 2 || seen[id as usize] {
                continue;
            }
            seen[id as usize] = true;
            output_references[id as usize] += 1;
            let n = dag.nodes[id as usize];
            stack.push(n.low);
            stack.push(n.high);
        }
    }
    let cross_output_shared_nodes = output_references.iter().skip(2).filter(|&&n| n > 1).count();
    let result = Profile {
        roots,
        input_terms,
        canonical_terms,
        literal_occurrences,
        naive_branch_bound,
        nontrivial_outputs,
        max_monomials_per_output,
        max_degree,
        shared_nodes,
        shared_edges,
        max_fanout,
        cross_output_shared_nodes,
        node_count: dag.nodes.len() - 2,
        reuse_hits: dag.reuse_hits,
        visits: dag.visits,
    };
    Ok((dag, result))
}
fn verify(problem: &Problem, dag: &Dag, roots: &[u32], samples: usize) -> Result<(), String> {
    for k in 0..samples {
        let x = (k as u64)
            .wrapping_mul(0x9e3779b97f4a7c15)
            .wrapping_add(0xbf58476d1ce4e5b9);
        let mut memo = vec![None; dag.nodes.len()];
        for (eq, &root) in problem.equations.iter().zip(roots) {
            if dag.evaluate(root, x, &mut memo) != oracle(eq, x) {
                return Err("ANF_DAG_SOURCE_EVAL_MISMATCH".into());
            }
        }
    }
    Ok(())
}

// Joint, *phase-free and unweighted* exact trace count via a bounded ROBDD.
// The polynomial DAG above is NOT itself a Boolean solution-set BDD.
const BDD_NODE_BUDGET: usize = 250_000;
const BDD_APPLY_BUDGET: usize = 3_000_000;
const BDD_CACHE_BUDGET: usize = 1_500_000;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum BddOp {
    Xor,
    And,
}

#[derive(Clone, Copy)]
struct BddNode {
    variable: usize,
    low: u32,
    high: u32,
}

struct Bdd {
    // BDD terminals: 0 = false, 1 = true.
    nodes: Vec<BddNode>,
    unique: HashMap<(usize, u32, u32), u32>,
    apply_cache: HashMap<(BddOp, u32, u32), u32>,
    polynomial_cache: HashMap<u32, u32>,
    apply_calls: usize,
    max_nodes: usize,
    max_calls: usize,
}

impl Bdd {
    fn new(max_nodes: usize, max_calls: usize) -> Self {
        Self {
            nodes: vec![
                BddNode { variable: 0, low: 0, high: 0 },
                BddNode { variable: 0, low: 0, high: 0 },
            ],
            unique: HashMap::new(),
            apply_cache: HashMap::new(),
            polynomial_cache: HashMap::new(),
            apply_calls: 0,
            max_nodes,
            max_calls,
        }
    }
    fn mk(&mut self, variable: usize, low: u32, high: u32) -> Result<u32, String> {
        if low == high {
            return Ok(low);
        }
        if let Some(&id) = self.unique.get(&(variable, low, high)) {
            return Ok(id);
        }
        if self.nodes.len() - 2 >= self.max_nodes {
            return Err("JOINT_BDD_NODE_LIMIT".into());
        }
        let id = u32::try_from(self.nodes.len()).map_err(|_| "JOINT_BDD_NODE_ID_LIMIT")?;
        self.nodes.push(BddNode { variable, low, high });
        self.unique.insert((variable, low, high), id);
        Ok(id)
    }
    fn var(&self, id: u32) -> usize {
        if id < 2 { usize::MAX } else { self.nodes[id as usize].variable }
    }
    fn cofactor(&self, id: u32, var: usize) -> (u32, u32) {
        if self.var(id) == var {
            let n = self.nodes[id as usize];
            (n.low, n.high)
        } else {
            (id, id)
        }
    }
    fn apply(&mut self, op: BddOp, mut a: u32, mut b: u32) -> Result<u32, String> {
        self.apply_calls = self.apply_calls.checked_add(1)
            .ok_or("JOINT_BDD_APPLY_OVERFLOW")?;
        if self.apply_calls > self.max_calls {
            return Err("JOINT_BDD_APPLY_LIMIT".into());
        }
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        let terminal = match op {
            BddOp::And if a == 0 => Some(0),
            BddOp::And if a == 1 => Some(b),
            BddOp::And if a == b => Some(a),
            BddOp::Xor if a == 0 => Some(b),
            BddOp::Xor if a == b => Some(0),
            _ => None,
        };
        if let Some(value) = terminal {
            return Ok(value);
        }
        let key = (op, a, b);
        if let Some(&value) = self.apply_cache.get(&key) {
            return Ok(value);
        }
        if self.apply_cache.len() >= BDD_CACHE_BUDGET {
            return Err("JOINT_BDD_CACHE_LIMIT".into());
        }
        let pivot = self.var(a).min(self.var(b));
        if pivot == usize::MAX {
            return Err("JOINT_BDD_INVALID_TERMINALS".into());
        }
        let (al, ah) = self.cofactor(a, pivot);
        let (bl, bh) = self.cofactor(b, pivot);
        let low = self.apply(op, al, bl)?;
        let high = self.apply(op, ah, bh)?;
        let result = self.mk(pivot, low, high)?;
        self.apply_cache.insert(key, result);
        Ok(result)
    }
    fn polynomial(&mut self, poly: &Dag, id: u32) -> Result<u32, String> {
        if id < 2 {
            return Ok(id);
        }
        if let Some(&bdd) = self.polynomial_cache.get(&id) {
            return Ok(bdd);
        }
        let n = poly.nodes[id as usize];
        let lo = self.polynomial(poly, n.low)?;
        let hi = self.polynomial(poly, n.high)?;
        // Polynomial cofactor identity:
        // f[x_v=0] = low, f[x_v=1] = low XOR high.
        let one = self.apply(BddOp::Xor, lo, hi)?;
        let result = self.mk(n.variable, lo, one)?;
        self.polynomial_cache.insert(id, result);
        Ok(result)
    }
    fn evaluate(&self, mut id: u32, assignment: u64) -> bool {
        while id >= 2 {
            let n = self.nodes[id as usize];
            id = if assignment & (1u64 << n.variable) == 0 {
                n.low
            } else {
                n.high
            };
        }
        id == 1
    }
    fn model_count(&self, root: u32, variables: usize) -> Result<u128, String> {
        if variables > 56 {
            return Err("JOINT_BDD_VARIABLE_LIMIT".into());
        }
        fn walk(
            bdd: &Bdd,
            id: u32,
            depth: usize,
            variables: usize,
            cache: &mut HashMap<(u32, usize), u128>,
        ) -> Result<u128, String> {
            match id {
                0 => return Ok(0),
                1 => return Ok(1u128 << (variables - depth)),
                _ => {}
            }
            if let Some(&v) = cache.get(&(id, depth)) {
                return Ok(v);
            }
            let node = bdd.nodes[id as usize];
            if node.variable < depth || node.variable >= variables {
                return Err("JOINT_BDD_INVALID_ORDER".into());
            }
            let low = walk(bdd, node.low, node.variable + 1, variables, cache)?;
            let high = walk(bdd, node.high, node.variable + 1, variables, cache)?;
            let count = (low + high) << (node.variable - depth);
            cache.insert((id, depth), count);
            Ok(count)
        }
        walk(self, root, 0, variables, &mut HashMap::new())
    }
}

struct JointCount {
    count: u128,
    sat: bool,
    polynomial_nodes: usize,
    bdd_nodes: usize,
    apply_calls: usize,
    cache_entries: usize,
}

fn joint_count(
    problem: &Problem,
    poly: &Dag,
    roots: &[u32],
    node_budget: usize,
    apply_budget: usize,
) -> Result<JointCount, String> {
    // Explicit contract: all weights are unit and the phase is absent.
    // For arbitrary phase/weight semantics there is NO certified shortcut.
    if problem.phase_terms != 0 {
        return Err("JOINT_BDD_PHASE_INELIGIBLE".into());
    }
    if problem.variables > MAX_VARIABLES {
        return Err("JOINT_BDD_VARIABLE_LIMIT".into());
    }
    if roots.len() != problem.equations.len() {
        return Err("JOINT_BDD_ROOT_CARDINALITY".into());
    }
    let mut bdd = Bdd::new(node_budget, apply_budget);
    let mut conjunction = 1u32;
    for &root in roots {
        if root == 0 { continue; }
        let polynomial = bdd.polynomial(poly, root)?;
        let constraint = bdd.apply(BddOp::Xor, polynomial, 1)?;
        conjunction = bdd.apply(BddOp::And, conjunction, constraint)?;
        if conjunction == 0 { break; }
    }
    let count = bdd.model_count(conjunction, problem.variables)?;
    // Sampled source witness catches severe variable remapping, complement,
    // free-variable or conjunction errors. This is not the WMC oracle.
    let trials = if problem.variables <= 12 {
        1usize << problem.variables
    } else {
        512
    };
    for trial in 0..trials {
        let assignment = if problem.variables <= 12 {
            trial as u64
        } else {
            (trial as u64)
                .wrapping_mul(0x9e3779b97f4a7c15)
                .wrapping_add(0xbf58476d1ce4e5b9)
        };
        let expected = problem.equations.iter().all(|eq| !oracle(eq, assignment));
        if bdd.evaluate(conjunction, assignment) != expected {
            return Err("JOINT_BDD_DIRECT_SOURCE_DISAGREEMENT".into());
        }
    }
    Ok(JointCount {
        count,
        sat: count != 0,
        polynomial_nodes: poly.nodes.len() - 2,
        bdd_nodes: bdd.nodes.len() - 2,
        apply_calls: bdd.apply_calls,
        cache_entries: bdd.apply_cache.len(),
    })
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let first = args
        .next()
        .ok_or("usage: [--joint-bdd] INPUT.anf [OUTPUT.tsv]")?;
    let (count_bdd, path) = if first == "--joint-bdd" {
        (true, args.next().ok_or("JOINT_BDD_MISSING_SOURCE")?)
    } else {
        (false, first)
    };
    let output = args.next();
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let t0 = Instant::now();
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let problem = parse_problem(&text)?;
    let parse_ns = t0.elapsed().as_nanos();
    let start = Instant::now();
    let (dag, report) = profile(&problem)?;
    let compile_ns = start.elapsed().as_nanos();
    if count_bdd {
        let begin = Instant::now();
        let result = joint_count(
            &problem, &dag, &report.roots, BDD_NODE_BUDGET, BDD_APPLY_BUDGET,
        )?;
        let count_ns = begin.elapsed().as_nanos();
        let header = "status\tvariables\tphase_terms\toutput_equations\tpolynomial_dag_nodes\tbdd_nodes\tapply_calls\tapply_cache_entries\tsat\twc_real\twc_imag\tsource_parse_ns\tpoly_compile_ns\tjoint_bdd_ns\n";
        let row = format!(
            "JOINT_BDD_COMPLETE\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0\t{}\t{}\t{}\n",
            problem.variables, problem.phase_terms, problem.equations.len(),
            result.polynomial_nodes, result.bdd_nodes, result.apply_calls,
            result.cache_entries, result.sat, result.count,
            parse_ns, compile_ns, count_ns,
        );
        if let Some(output) = output {
            fs::write(output, format!("{header}{row}"))
                .map_err(|e| e.to_string())?;
        }
        print!("{header}{row}");
        return Ok(());
    }
    let start = Instant::now();
    verify(&problem, &dag, &report.roots, 256)?;
    let verify_ns = start.elapsed().as_nanos();
    let status = if problem.phase_terms == 0 {
        "PHASE_FREE_PROFILE_ONLY"
    } else {
        "PHASE_PRESENT_PROFILE_ONLY"
    };
    let header = "status\tvariables\tphase_terms\toutput_equations\tnontrivial_output_equations\tinput_anf_terms\tcanonical_trace_terms\tcanonical_literal_occurrences\tnaive_branch_upper_bound\tunique_dag_nodes\tshared_dag_nodes\tcross_output_shared_nodes\tshared_extra_edges\tmax_dag_fanout\tinterning_hits\tmask_visits\tmax_trace_terms_per_output\tmax_trace_degree\tparse_ns\tcompile_ns\tverify_ns\n";
    let line = format!(
        "{status}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        problem.variables, problem.phase_terms, problem.equations.len(),
        report.nontrivial_outputs, report.input_terms, report.canonical_terms,
        report.literal_occurrences, report.naive_branch_bound,
        report.node_count, report.shared_nodes, report.cross_output_shared_nodes,
        report.shared_edges, report.max_fanout, report.reuse_hits,
        report.visits, report.max_monomials_per_output, report.max_degree,
        parse_ns, compile_ns, verify_ns
    );
    if let Some(output) = output {
        fs::write(output, format!("{header}{line}")).map_err(|e| e.to_string())?;
    }
    print!("{header}{line}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("ANF_DAG_RESEARCH_CENSORED {e}");
        std::process::exit(2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_zero_and_duplicate_monomials_cancel_in_gf2() {
        let p = parse_problem("p anf 4 2 0\no 0 3\nm 1 0\nm 2 1 2\nm 2 2 1\no 3 2\nm 1 3\nm 0\n")
            .unwrap();
        let (dag, result) = profile(&p).unwrap();
        assert_eq!(result.nontrivial_outputs, 1);
        assert_eq!(result.roots[0], 0);
        verify(&p, &dag, &result.roots, 128).unwrap();
    }
    #[test]
    fn independent_small_truth_tables_with_nontrivial_shared_outputs() {
        for seed in 0usize..40 {
            let mut equations = Vec::new();
            for output in 0..4 {
                let mut terms = Vec::new();
                terms.push(vec![output]);
                for j in 0..10 {
                    let mut term = Vec::new();
                    for v in 0..7 {
                        if (seed * 17 + j * 23 + output * 11 + v * 13) >> (v % 4) & 1 != 0 {
                            term.push(v);
                        }
                    }
                    terms.push(term);
                }
                equations.push(Equation { output, terms });
            }
            let p = Problem {
                variables: 7,
                phase_terms: 0,
                equations,
            };
            let (dag, r) = profile(&p).unwrap();
            for assignment in 0..128u64 {
                for (eq, &root) in p.equations.iter().zip(&r.roots) {
                    let mut memo = vec![None; dag.nodes.len()];
                    assert_eq!(
                        dag.evaluate(root, assignment, &mut memo),
                        oracle(eq, assignment),
                        "seed={seed}, assignment={assignment}"
                    );
                }
            }
        }
    }
    #[test]
    fn reject_invalid_source_and_budgets() {
        for s in [
            "p anf 3 1 0\no 3 0\n",
            "p anf 3 1 0\no 1 1\nm 2 1\n",
            "p anf 3 1 0\no 1 1\nm 1 4\n",
            "p anf 3 1 0\no 1 0\nx 1\n",
            "p anf 57 0 0\n",
        ] {
            assert!(parse_problem(s).is_err(), "{s}");
        }
    }
    #[test]
    fn joint_bdd_exact_independent_multiequation_counts() {
        for seed in 0..30usize {
            let mut equations = Vec::new();
            for output in 0..5usize {
                let mut terms = vec![vec![output]];
                for k in 0..10usize {
                    let variables: Vec<usize> = (0..7)
                        .filter(|&v| ((seed * 31 + output * 17 + k * 13 + v * 7)
                                    >> (v % 3)) & 1 == 1)
                        .collect();
                    terms.push(variables);
                }
                equations.push(Equation { output, terms });
            }
            let problem = Problem { variables: 7, phase_terms: 0, equations };
            let (poly, profile) = profile(&problem).unwrap();
            let result = joint_count(&problem, &poly, &profile.roots, 25_000, 250_000).unwrap();
            let oracle_count = (0u64..128)
                .filter(|&x| problem.equations.iter().all(|eq| !oracle(eq, x)))
                .count() as u128;
            assert_eq!(result.count, oracle_count, "seed={seed}");
            assert_eq!(result.sat, oracle_count != 0);
        }
    }
    #[test]
    fn joint_bdd_tautology_constant_and_free_vars() {
        let problem = parse_problem(
            "p anf 6 2 0\no 0 1\nm 1 0\no 1 2\nm 1 2\nm 0\n",
        ).unwrap();
        let (poly, profile) = profile(&problem).unwrap();
        let result = joint_count(&problem, &poly, &profile.roots, 2000, 5000).unwrap();
        // x1 XOR x2 XOR 1 ==0 and the remaining 4 bits are free.
        assert_eq!(result.count, 32);
        assert!(result.sat);
    }
    #[test]
    fn joint_bdd_rejects_phases_and_excess_node_budget() {
        let p = parse_problem("p anf 4 1 1\no 0 1\nm 1 1\n").unwrap();
        let (d, r) = profile(&p).unwrap();
        assert!(joint_count(&p, &d, &r.roots, 100, 1000).is_err());
        let p = parse_problem(
            "p anf 4 2 0\no 0 2\nm 2 1 2\nm 2 2 3\no 1 1\nm 1 3\n",
        ).unwrap();
        let (d, r) = profile(&p).unwrap();
        let error = joint_count(&p, &d, &r.roots, 1, 1000).err().unwrap();
        assert_eq!(error, "JOINT_BDD_NODE_LIMIT");
        let error = joint_count(&p, &d, &r.roots, 1000, 1).err().unwrap();
        assert_eq!(error, "JOINT_BDD_APPLY_LIMIT");
    }
    #[test]
    fn hashcons_shares_suffix_polynomials() {
        let p = parse_problem("p anf 6 2 0\no 0 4\nm 1 0\nm 2 1 2\nm 2 1 3\nm 2 1 4\no 5 4\nm 1 5\nm 2 1 2\nm 2 1 3\nm 2 1 4\n").unwrap();
        let (dag, report) = profile(&p).unwrap();
        assert_eq!(report.roots[0], report.roots[1]);
        assert!(report.cross_output_shared_nodes > 0);
        assert!(report.reuse_hits > 0);
        verify(&p, &dag, &report.roots, 256).unwrap();
    }
}
