//! Temporary pinned-QuPRS research example. Exports exact post-RR Boolean
//! trace ANFs only; it does not solve WMC or change any release backend.
use quprs::{qasm, PathSum, VerificationStrategy};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn source_path(case: &str) -> Option<&'static str> {
    match case {
        "feynman_adder8" => Some("benchmarks/Feynman/adder_8.qasm"),
        "feynman_tof10" => Some("benchmarks/Feynman/tof_10.qasm"),
        "feynman_barenco10" => Some("benchmarks/Feynman/barenco_tof_10.qasm"),
        "feynman_hwb6" => Some("benchmarks/Feynman/hwb6.qasm"),
        _ => None,
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let case = args.next().ok_or("case required")?;
    let root = args.next().ok_or("pinned source root required")?;
    let target = args.next().ok_or("source ANF output file required")?;
    if args.next().is_some() {
        return Err("unexpected extra arguments".into());
    }
    let case = case.to_str().ok_or("invalid source case encoding")?;
    let rel = source_path(case).ok_or("not in pinned ANF research case allowlist")?;
    let input = fs::read_to_string(Path::new(&root).join(rel))?;
    let started = Instant::now();
    let (ops, qubits) = qasm::parse_qasm_str(&input)?;
    if !(1..=24).contains(&qubits) {
        return Err("source qubits outside bounded research range".into());
    }
    let mut base = PathSum::new(qubits);
    base.set_auto_reduce(true);
    let ps = VerificationStrategy::Naive.run(base, &[], &ops);
    if ps.rr_approximate {
        return Err("ANF_SOURCE_REQUIRES_EXACT_RR".into());
    }
    let rr_ns = started.elapsed().as_nanos();
    // Preserve all original Boolean variables, including otherwise
    // unused/free paths; IDs are remapped in a stable sorted order.
    let mut ids: BTreeSet<u32> = ps.v.path_vars.iter().copied().collect();
    ids.extend((0..qubits).map(|v| v as u32));
    for monomial in ps.p.terms.keys() {
        ids.extend(monomial.iter().copied());
    }
    for equation in &ps.f.functions {
        for monomial in equation {
            ids.extend(monomial.iter().copied());
        }
    }
    if ids.len() > 56 {
        return Err("ANF_SOURCE_VARIABLE_LIMIT".into());
    }
    let map: BTreeMap<u32, usize> = ids.iter().enumerate().map(|(i, &old)| (old, i)).collect();
    let mut text = format!(
        "p anf {} {} {}\n",
        ids.len(),
        ps.f.functions.len(),
        ps.p.terms.len()
    );
    let mut total_terms = 0usize;
    let mut max_degree = 0usize;
    for (output, equation) in ps.f.functions.iter().enumerate() {
        total_terms += equation.len();
        text.push_str(&format!("o {} {}\n", map[&(output as u32)], equation.len()));
        for monomial in equation {
            let mut vars: Vec<_> = monomial.iter().map(|v| map[v]).collect();
            vars.sort_unstable();
            vars.dedup();
            max_degree = max_degree.max(vars.len());
            text.push_str(&format!("m {}", vars.len()));
            for v in vars {
                text.push_str(&format!(" {v}"));
            }
            text.push('\n');
        }
    }
    // No phase content is serialized; phase count explicitly signals
    // phase-bearing sources are *not* eligible for counting by this DAG.
    fs::write(target, text)?;
    println!(
        "PINNED_QU_PRS_ANF_EXPORT case={case} qubits={qubits} gates={} \
         rr_variables={} phase_terms={} output_equations={} source_anf_terms={} \
         source_max_degree={max_degree} source_rr_ns={rr_ns}",
        ops.len(),
        ids.len(),
        ps.p.terms.len(),
        ps.f.functions.len(),
        total_terms
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("PINNED_QU_PRS_ANF_EXPORT_CENSORED {error}");
        std::process::exit(2);
    }
}
