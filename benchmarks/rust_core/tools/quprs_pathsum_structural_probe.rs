//! Source-pinned, standalone QuPRS *post-Reduction* structural probe.
//! Copy into the pinned QuPRS examples/ in an isolated CI checkout only.
//! This does not change QuPRS's exported WMC solver or original QASM.
use quprs::{qasm, PathSum, VerificationStrategy};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn source_path(case: &str) -> Option<&'static str> {
    match case {
        "feynman_hwb6" => Some("benchmarks/Feynman/hwb6.qasm"),
        "feynman_qft4" => Some("benchmarks/Feynman/qft_4.qasm"),
        "feynman_barenco_tof_5" => Some("benchmarks/Feynman/barenco_tof_5.qasm"),
        "feynman_mod_mult_55" => Some("benchmarks/Feynman/mod_mult_55.qasm"),
        "mqt_vqe_4" => Some("benchmarks/MQTBench/vqe_nativegates_ibm_qiskit_opt0_4.qasm"),
        "mqt_qnn_4" => Some("benchmarks/MQTBench/qnn_nativegates_ibm_qiskit_opt0_4.qasm"),
        "mqt_grover_4" => Some("benchmarks/MQTBench/grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm"),
        _ => None,
    }
}

fn min_degree_width(scopes: &[Vec<u32>], all_vars: &BTreeSet<u32>) -> Option<usize> {
    // Explicit research budget. Excessive width analysis is not admissible,
    // never reported as "small width" merely because it was not computed.
    if all_vars.len() > 256 {
        return None;
    }
    let mut graph: BTreeMap<u32, BTreeSet<u32>> =
        all_vars.iter().map(|&v| (v, BTreeSet::new())).collect();
    for scope in scopes {
        let s: BTreeSet<u32> = scope.iter().copied().collect();
        for &a in &s {
            for &b in &s {
                if a != b {
                    graph.get_mut(&a)?.insert(b);
                }
            }
        }
    }
    let mut width = 0usize;
    while !graph.is_empty() {
        let pivot = *graph.iter().min_by_key(|(v, adj)| (adj.len(), **v))?.0;
        let adjacent: Vec<u32> = graph.get(&pivot)?.iter().copied().collect();
        width = width.max(adjacent.len());
        for (i, &a) in adjacent.iter().enumerate() {
            for &b in adjacent.iter().skip(i + 1) {
                graph.get_mut(&a)?.insert(b);
                graph.get_mut(&b)?.insert(a);
            }
        }
        for &neighbor in &adjacent {
            graph.get_mut(&neighbor)?.remove(&pivot);
        }
        graph.remove(&pivot);
    }
    Some(width)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let case_os = args.next().ok_or("case ID required")?;
    let root_os = args.next().ok_or("QuPRS root required")?;
    let mode_os = args.next().ok_or("mode: raw or rr")?;
    let output_os = args.next().ok_or("output TSV path required")?;
    if args.next().is_some() {
        return Err("unexpected extra arguments".into());
    }
    let case = case_os.to_str().ok_or("case must be UTF-8")?;
    let mode = mode_os.to_str().ok_or("mode must be UTF-8")?;
    if mode != "raw" && mode != "rr" {
        return Err("mode must be raw or rr".into());
    }
    let rel = source_path(case).ok_or("unsupported source case")?;
    let source = fs::read_to_string(Path::new(&root_os).join(rel))?;
    let parse_start = Instant::now();
    let (ops, qubits) = qasm::parse_qasm_str(&source)?;
    let qasm_parse_ns = parse_start.elapsed().as_nanos();
    if qubits == 0 || qubits > 12 {
        return Err("unsupported original QASM qubit count".into());
    }
    let pathsum_start = Instant::now();
    let mut base = PathSum::new(qubits);
    // Only change the reduction setting — NEVER modify source gates.
    base.set_auto_reduce(mode == "rr");
    let ps = VerificationStrategy::Naive.run(base, &[], &ops);
    let pathsum_build_ns = pathsum_start.elapsed().as_nanos();
    let analysis_start = Instant::now();

    let mut ids: BTreeSet<u32> = ps.v.path_vars.iter().copied().collect();
    ids.extend((0..qubits).map(|x| x as u32));

    let mut constraints = Vec::new();
    let mut max_anf_degree = 0;
    let mut output_xor_terms = 0;
    let mut output_and_terms = 0;
    for (output, terms) in ps.f.functions.iter().enumerate() {
        let mut scope = vec![output as u32];
        output_xor_terms += terms.len().saturating_sub(1);
        for monomial in terms {
            max_anf_degree = max_anf_degree.max(monomial.len());
            if monomial.len() > 1 {
                output_and_terms += 1;
            }
            scope.extend(monomial.iter().copied());
            ids.extend(monomial.iter().copied());
        }
        constraints.push(scope);
    }

    let mut max_phase_degree = 0;
    let mut phase_nonempty = 0;
    let mut phase_radians = 0;
    let mut phase_symbolic = 0;
    let mut phase_nonsign = 0;
    for (monomial, coeff) in &ps.p.terms {
        max_phase_degree = max_phase_degree.max(monomial.len());
        if !monomial.is_empty() {
            phase_nonempty += 1;
        }
        if coeff.radians.is_some() {
            phase_radians += 1;
        }
        if !coeff.symbols.is_empty() {
            phase_symbolic += 1;
        }
        let d = coeff.constant.denom;
        if d == 0 || (2 * i128::from(coeff.constant.numer)) % i128::from(d) != 0 {
            phase_nonsign += 1;
        }
        ids.extend(monomial.iter().copied());
        constraints.push(monomial.clone());
    }
    let max_scope = constraints
        .iter()
        .map(|s| s.iter().copied().collect::<BTreeSet<_>>().len())
        .max()
        .unwrap_or(0);
    let width = min_degree_width(&constraints, &ids);
    let sign_candidate = phase_radians == 0
        && phase_symbolic == 0
        && phase_nonsign == 0
        && max_phase_degree <= 2
        && max_anf_degree <= 1;
    let width_repr = width.map_or_else(|| "NA_WIDTH_BUDGET".to_owned(), |v| v.to_string());
    let structure_ns = analysis_start.elapsed().as_nanos();
    let report = format!(
        "case\tmode\tqubits\tgate_ops\tpath_vars\tstructural_vars\tphase_terms\tphase_nonempty\tphase_max_degree\tphase_radians\tphase_symbolic\tphase_nonsign\toutput_eqs\toutput_xor_links\toutput_and_terms\toutput_max_degree\tmax_native_factor_scope\tprimal_min_degree_width_upper\tsign_quadratic_candidate\tqasm_parse_ns\tpathsum_build_ns\tstructural_analysis_ns\n\
         {case}\t{mode}\t{qubits}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{width_repr}\t{}\t{qasm_parse_ns}\t{pathsum_build_ns}\t{structure_ns}\n",
        ops.len(),
        ps.v.path_vars.len(),
        ids.len(),
        ps.p.terms.len(),
        phase_nonempty,
        max_phase_degree,
        phase_radians,
        phase_symbolic,
        phase_nonsign,
        ps.f.functions.len(),
        output_xor_terms,
        output_and_terms,
        max_anf_degree,
        max_scope,
        sign_candidate
    );
    fs::write(Path::new(&output_os), report.as_bytes())?;
    println!(
        "PATHSUM_STRUCTURE case={case} mode={mode} vars={} phases={} phase_degree={} \
         output_anf_degree={} factor_scope={} width_upper={} sign_quadratic_candidate={} \
         pathsum_build_ns={pathsum_build_ns}",
        ids.len(),
        ps.p.terms.len(),
        max_phase_degree,
        max_anf_degree,
        max_scope,
        width_repr,
        sign_candidate
    );
    Ok(())
}
