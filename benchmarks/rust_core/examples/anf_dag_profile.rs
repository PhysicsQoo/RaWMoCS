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
    raw.parse::<usize>().map_err(|_| "invalid ANF integer".into())
}
fn parse_problem(raw: &str) -> Result<Problem, String> {
    if raw.len() > 8_000_000 {
        return Err("ANF_SOURCE_BYTE_LIMIT".into());
    }
    let lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
    let head: Vec<&str> = lines.first().ok_or("ANF_SOURCE_EMPTY")?
        .split_whitespace().collect();
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
        let row: Vec<&str> = lines.get(index).ok_or("ANF_MISSING_EQUATION")?
            .split_whitespace().collect();
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
            let fields: Vec<&str> = lines.get(index).ok_or("ANF_MISSING_MONOMIAL")?
                .split_whitespace().collect();
            index += 1;
            if fields.len() < 2 || fields[0] != "m" {
                return Err("ANF_MONOMIAL_HEADER".into());
            }
            let degree = integer(fields[1])?;
            if degree > variables || degree + 2 != fields.len() {
                return Err("ANF_MONOMIAL_DEGREE".into());
            }
            let term = fields[2..].iter()
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
    Ok(Problem { variables, phase_terms, equations })
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
                Node { variable: 0, low: 0, high: 0 },
                Node { variable: 0, low: 0, high: 0 },
            ],
            unique: HashMap::new(),
            visits: 0,
            reuse_hits: 0,
        }
    }
    fn compile(&mut self, terms: &[u64]) -> Result<u32, String> {
        self.visits = self.visits.checked_add(terms.len())
            .ok_or("ANF_WORK_COUNTER_OVERFLOW")?;
        if self.visits > MAX_MASK_VISITS {
            return Err("ANF_WORK_LIMIT".into());
        }
        if terms.is_empty() { return Ok(0); }
        if terms.len() == 1 && terms[0] == 0 { return Ok(1); }
        let pivot = terms.iter().filter(|&&m| m != 0)
            .map(|m| m.trailing_zeros() as usize).min()
            .ok_or("ANF_INVALID_MONOMIAL_SET")?;
        let bit = 1u64 << pivot;
        let mut lows = Vec::new();
        let mut highs = Vec::new();
        for &m in terms {
            if m & bit == 0 { lows.push(m); }
            else { highs.push(m ^ bit); }
        }
        // Both partitions preserve sortedness because bit-removal has
        // no collisions on the high branch for distinct original masks.
        let low = self.compile(&lows)?;
        let high = self.compile(&highs)?;
        if high == 0 { return Ok(low); }
        let key = (pivot, low, high);
        if let Some(&id) = self.unique.get(&key) {
            self.reuse_hits += 1;
            return Ok(id);
        }
        if self.nodes.len() >= MAX_UNIQUE_NODES {
            return Err("ANF_DAG_NODE_LIMIT".into());
        }
        let id = u32::try_from(self.nodes.len()).map_err(|_| "ANF_DAG_ID_LIMIT")?;
        self.nodes.push(Node { variable: pivot, low, high });
        self.unique.insert(key, id);
        Ok(id)
    }
    fn evaluate(&self, root: u32, assignment: u64, memo: &mut [Option<bool>]) -> bool {
        match root {
            0 => false,
            1 => true,
            _ => {
                let i = root as usize;
                if let Some(value) = memo[i] { return value; }
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
        max_degree = max_degree.max(terms.iter().map(|m| m.count_ones() as usize).max().unwrap_or(0));
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
            if id < 2 || seen[id as usize] { continue; }
            seen[id as usize] = true;
            output_references[id as usize] += 1;
            let n = dag.nodes[id as usize];
            stack.push(n.low);
            stack.push(n.high);
        }
    }
    let cross_output_shared_nodes =
        output_references.iter().skip(2).filter(|&&n| n > 1).count();
    let result = Profile {
        roots, input_terms, canonical_terms, literal_occurrences,
        naive_branch_bound, nontrivial_outputs, max_monomials_per_output,
        max_degree, shared_nodes, shared_edges, max_fanout,
        cross_output_shared_nodes, node_count: dag.nodes.len() - 2,
        reuse_hits: dag.reuse_hits, visits: dag.visits,
    };
    Ok((dag, result))
}
fn verify(problem: &Problem, dag: &Dag, roots: &[u32], samples: usize) -> Result<(), String> {
    for k in 0..samples {
        let x = (k as u64)
            .wrapping_mul(0x9e3779b97f4a7c15)
            .wrapping_add(0xbf58476d1ce4e5b9);
        for (eq, &root) in problem.equations.iter().zip(roots) {
            let mut memo = vec![None; dag.nodes.len()];
            if dag.evaluate(root, x, &mut memo) != oracle(eq, x) {
                return Err("ANF_DAG_SOURCE_EVAL_MISMATCH".into());
            }
        }
    }
    Ok(())
}
fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let path = args.next().ok_or("usage: anf_dag_profile INPUT.anfs [OUTPUT.tsv]")?;
    let output = args.next();
    if args.next().is_some() { return Err("unexpected extra argument".into()); }
    let t0 = Instant::now();
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let problem = parse_problem(&text)?;
    let parse_ns = t0.elapsed().as_nanos();
    let start = Instant::now();
    let (dag, report) = profile(&problem)?;
    let compile_ns = start.elapsed().as_nanos();
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
        "{status}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
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
        let p = parse_problem("p anf 4 2 0\no 0 3\nm 1 0\nm 2 1 2\nm 2 2 1\no 3 2\nm 1 3\nm 0\n").unwrap();
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
            let p = Problem { variables: 7, phase_terms: 0, equations };
            let (dag, r) = profile(&p).unwrap();
            for assignment in 0..128u64 {
                for (eq, &root) in p.equations.iter().zip(&r.roots) {
                    let mut memo = vec![None; dag.nodes.len()];
                    assert_eq!(dag.evaluate(root, assignment, &mut memo),
                               oracle(eq, assignment), "seed={seed}, assignment={assignment}");
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
    fn hashcons_shares_suffix_polynomials() {
        let p = parse_problem("p anf 6 2 0\no 0 4\nm 1 0\nm 2 1 2\nm 2 1 3\nm 2 1 4\no 5 4\nm 1 5\nm 2 1 2\nm 2 1 3\nm 2 1 4\n").unwrap();
        let (dag, report) = profile(&p).unwrap();
        assert_eq!(report.roots[0], report.roots[1]);
        assert!(report.cross_output_shared_nodes > 0);
        assert!(report.reuse_hits > 0);
        verify(&p, &dag, &report.roots, 256).unwrap();
    }
}
