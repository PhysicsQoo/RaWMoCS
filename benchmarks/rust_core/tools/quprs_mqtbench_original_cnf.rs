//! Source-locked MQT Bench QASM → QuPRS WMC CNF research driver.
//! Reads existing unmodified committed MQTBench QASMs in pinned QuPRS.
//! This driver runs in a temporary CI checkout, calling the unchanged
//! pinned QuPRS public PathSum/WmcManager API; does not alter production.
//! The resulting CNFs are QuPRS-generated from MQTBench circuits, not
//! files claimed to have been directly published as CNF by MQT Bench.
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use quprs::{qasm, PathSum, VerificationStrategy, wmc::WmcManager};

const IDS: &[&str] = &[
    "mqt_wstate_4", "mqt_vqe_4", "mqt_qnn_4",
    "mqt_grover_4", "mqt_qwalk_3", "mqt_qaoa_7", "mqt_qpeexact_8",
];
fn source_for(case: &str, quprs_root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let basename = match case {
        "mqt_wstate_4" => "wstate_nativegates_ibm_qiskit_opt0_4.qasm",
        "mqt_vqe_4" => "vqe_nativegates_ibm_qiskit_opt0_4.qasm",
        "mqt_qnn_4" => "qnn_nativegates_ibm_qiskit_opt0_4.qasm",
        "mqt_grover_4" => "grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm",
        "mqt_qwalk_3" => "qwalk-noancilla_nativegates_ibm_qiskit_opt0_3.qasm",
        "mqt_qaoa_7" => "qaoa_nativegates_ibm_qiskit_opt0_7.qasm",
        "mqt_qpeexact_8" => "qpeexact_nativegates_ibm_qiskit_opt0_8.qasm",
        _ => return Err(format!("unsupported MQTBench original case: {case}").into()),
    };
    // Never translate, transpile, drop measurement, or rewrite the input.
    // The pinned existing QuPRS MQTBench corpus is the source of truth.
    Ok(fs::read_to_string(
        quprs_root.join("benchmarks").join("MQTBench").join(basename)
    )?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut argv = env::args().skip(1);
    let case = argv.next().ok_or("required case ID")?;
    let root = argv.next().ok_or("required QuPRS source checkout root")?;
    let out = argv.next().ok_or("required output directory")?;
    if argv.next().is_some() || !IDS.contains(&case.as_str()) {
        return Err("unsupported MQTBench case/extra args".into());
    }
    let source = source_for(&case, Path::new(&root))?;
    let folder = Path::new(&out);
    fs::create_dir_all(folder)?;
    let (ops, qubits) = qasm::parse_qasm_str(&source)?;
    if qubits == 0 || qubits > 9 { return Err("unsupported MQTBench qubit count".into()); }
    let mut base = PathSum::new(qubits);
    base.set_auto_reduce(false);
    let ps = VerificationStrategy::Naive.run(base, &[], &ops);
    let paths = ps.v.path_vars.len().checked_sub(qubits).ok_or("path count underflow")?;
    let mut manager = WmcManager::new(&ps);
    manager.encode_trace(&ps)?;
    // Same phase-numbering algorithm as the pinned original QuPRS exporter.
    let mut vars: HashMap<u32, i32> = HashMap::new();
    let mut next = 1i32;
    let mut phases = String::from("weight_lit\tphase_numerator\tphase_denominator\tmonomial\n");
    for (monomial, coefficient) in &ps.p.terms {
        if !coefficient.symbols.is_empty() { return Err("symbolic coefficients unsupported".into()); }
        let weight_var = next;
        next += 1;
        for &path in monomial {
            if let std::collections::hash_map::Entry::Vacant(entry) = vars.entry(path) {
                entry.insert(next);
                next += 1;
            }
        }
        let mono = monomial.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        phases.push_str(&format!(
            "{weight_var}\t{}\t{}\t{mono}\n",
            coefficient.constant.numer, coefficient.constant.denom
        ));
    }
    fs::write(folder.join(format!("{case}.qasm")), source.as_bytes())?;
    for dialect in ["gpmc", "ganak"] {
        // Exactly the same QuPRS manager as regular WMC-only verification.
        fs::write(folder.join(format!("{case}-{dialect}.cnf")),
                  manager.to_dimacs_string(dialect).as_bytes())?;
        fs::write(folder.join(format!("{case}-{dialect}-phases.tsv")), phases.as_bytes())?;
    }
    println!(
        "MQTBENCH_QU_PRS_NATIVE_ENCODER case={case} qubits={qubits} gates={} pathvars={paths} phase_terms={}",
        ops.len(), ps.p.terms.len()
    );
    Ok(())
}
