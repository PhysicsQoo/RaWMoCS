//! Source-locked, resource-bounded hard-case exporter for ORIGINAL QuPRS
//! Feynman/MQTBench QASM. This research-only bridge is copied into the
//! pinned QuPRS example checkout; it never rewrites QASM or solver sources.
use std::env;
use std::fs;
use std::path::Path;
use quprs::{qasm, PathSum, VerificationStrategy, wmc::WmcManager};

const SOURCES: &[(&str, &str)] = &[
    ("feynman_barenco_tof_10", "benchmarks/Feynman/barenco_tof_10.qasm"),
    ("feynman_gf2_6_mult", "benchmarks/Feynman/gf2^6_mult.qasm"),
    ("feynman_gf2_8_mult", "benchmarks/Feynman/gf2^8_mult.qasm"),
    ("feynman_gf2_10_mult", "benchmarks/Feynman/gf2^10_mult.qasm"),
    ("feynman_adder_8", "benchmarks/Feynman/adder_8.qasm"),
    ("feynman_ham15_med", "benchmarks/Feynman/ham15-med.qasm"),
    ("feynman_qcla_mod_7", "benchmarks/Feynman/qcla_mod_7.qasm"),
    ("mqt_qnn_8", "benchmarks/MQTBench/qnn_nativegates_ibm_qiskit_opt0_8.qasm"),
    ("mqt_qnn_12", "benchmarks/MQTBench/qnn_nativegates_ibm_qiskit_opt0_12.qasm"),
    ("mqt_grover_5", "benchmarks/MQTBench/grover-noancilla_nativegates_ibm_qiskit_opt0_5.qasm"),
    ("mqt_grover_6", "benchmarks/MQTBench/grover-noancilla_nativegates_ibm_qiskit_opt0_6.qasm"),
    ("mqt_qwalk_5", "benchmarks/MQTBench/qwalk-noancilla_nativegates_ibm_qiskit_opt0_5.qasm"),
    ("mqt_qaoa_11", "benchmarks/MQTBench/qaoa_nativegates_ibm_qiskit_opt0_11.qasm"),
    ("mqt_qpeexact_16", "benchmarks/MQTBench/qpeexact_nativegates_ibm_qiskit_opt0_16.qasm"),
    ("mqt_qft_16", "benchmarks/MQTBench/qft_nativegates_ibm_qiskit_opt0_16.qasm"),
    ("mqt_wstate_16", "benchmarks/MQTBench/wstate_nativegates_ibm_qiskit_opt0_16.qasm"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let case = args.next().ok_or("expected case or --list-sources")?;
    if case == "--list-sources" {
        if args.next().is_some() { return Err("extra --list-sources arguments".into()); }
        for (id, path) in SOURCES {
            println!("{id}\t{path}");
        }
        return Ok(());
    }
    let source_root = args.next().ok_or("missing pinned QuPRS checkout path")?;
    let output_root = args.next().ok_or("missing output folder")?;
    if args.next().is_some() { return Err("extra export arguments".into()); }
    let (_, source_relpath) = SOURCES.iter()
        .find(|(id, _)| *id == case)
        .ok_or("unlisted original benchmark source")?;
    let input = fs::read_to_string(Path::new(&source_root).join(source_relpath))?;
    let (operations, qubits) = qasm::parse_qasm_str(&input)?;
    if qubits == 0 || qubits > 24 {
        return Err("source QASM qubit count outside bounded research envelope".into());
    }
    let mut base = PathSum::new(qubits);
    base.set_auto_reduce(false);
    let paths = VerificationStrategy::Naive.run(base, &[], &operations);
    let path_variables = paths.v.path_vars.len()
        .checked_sub(qubits).ok_or("invalid original path variable count")?;
    let mut manager = WmcManager::new(&paths);
    manager.encode_trace(&paths)?;
    let cnf = manager.to_dimacs_string("gpmc");
    fs::create_dir_all(&output_root)?;
    fs::write(Path::new(&output_root).join(format!("{case}.qasm")), input.as_bytes())?;
    fs::write(Path::new(&output_root).join(format!("{case}-gpmc.cnf")), cnf.as_bytes())?;
    println!(
        "LONG_TAIL_ORIGINAL_SOURCE case={case} qubits={qubits} gates={} pathvars={path_variables} cnf_bytes={}",
        operations.len(), cnf.len()
    );
    Ok(())
}
