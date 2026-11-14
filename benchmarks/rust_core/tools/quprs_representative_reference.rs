//! QuPRS encoder comparison research DRIVER, distinct from QuPRS original six-case exporter.
//! It calls the UNMODIFIED pinned QuPRS public Rust PathSum and WmcManager.
//! This temporary example is injected only into a CI checkout; neither
//! QuPRS's solver source nor its public APIs are modified.
//! Research generated families are not mislabeled as MQT benchmark QASM.
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use quprs::{qasm, PathSum, VerificationStrategy, wmc::WmcManager};

const IDS: &[&str] = &[
    "ghz2", "ghz3", "ghz4", "graph3", "bv3", "qft2",
    "qft3", "grover2", "qaoa3", "feynman_mod5_4", "feynman_qft_4",
];

fn source_for(case: &str, quprs_root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    if case == "feynman_mod5_4" || case == "feynman_qft_4" {
        let file = if case == "feynman_mod5_4" {
            "benchmarks/Feynman/mod5_4.qasm"
        } else {
            "benchmarks/Feynman/qft_4.qasm"
        };
        // No transpilation or circuit rewriting: native committed QASM bytes.
        return Ok(fs::read_to_string(quprs_root.join(file))?);
    }
    let (qubits, gates) = match case {
        "ghz2" => (2, "h q[0];\ncx q[0],q[1];\n"),
        "ghz3" => (3, "h q[0];\ncx q[0],q[1];\ncx q[1],q[2];\n"),
        "ghz4" => (4, "h q[0];\ncx q[0],q[1];\ncx q[1],q[2];\ncx q[2],q[3];\n"),
        "graph3" => (3, "h q[0];\nh q[1];\nh q[2];\ncz q[0],q[1];\ncz q[1],q[2];\n"),
        "bv3" => (3, "x q[2];\nh q[0];\nh q[1];\nh q[2];\ncx q[0],q[2];\ncx q[1],q[2];\nh q[0];\nh q[1];\n"),
        "qft2" => (2, "h q[0];\ncp(pi/2) q[1],q[0];\nh q[1];\nswap q[0],q[1];\n"),
        "qft3" => (3, "h q[0];\ncp(pi/2) q[1],q[0];\ncp(pi/4) q[2],q[0];\nh q[1];\ncp(pi/2) q[2],q[1];\nh q[2];\nswap q[0],q[2];\n"),
        "grover2" => (2, "h q[0];\nh q[1];\ncz q[0],q[1];\nh q[0];\nh q[1];\nx q[0];\nx q[1];\nh q[1];\ncx q[0],q[1];\nh q[1];\nx q[0];\nx q[1];\nh q[0];\nh q[1];\n"),
        "qaoa3" => (3, "h q[0];\nh q[1];\nh q[2];\ncx q[0],q[1];\nrz(0.7) q[1];\ncx q[0],q[1];\ncx q[1],q[2];\nrz(0.7) q[2];\ncx q[1],q[2];\nrx(0.3) q[0];\nrx(0.3) q[1];\nrx(0.3) q[2];\n"),
        _ => return Err(format!("unknown family {case}").into()),
    };
    Ok(format!("OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[{qubits}];\n{gates}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut argv = env::args().skip(1);
    let case = argv.next().ok_or("required case ID")?;
    let root = argv.next().ok_or("required QuPRS source checkout root")?;
    let out = argv.next().ok_or("required output directory")?;
    if argv.next().is_some() || !IDS.contains(&case.as_str()) {
        return Err("unsupported research case/extra args".into());
    }
    let source = source_for(&case, Path::new(&root))?;
    let folder = Path::new(&out);
    fs::create_dir_all(folder)?;
    let (ops, qubits) = qasm::parse_qasm_str(&source)?;
    if qubits == 0 || qubits > 9 { return Err("unexpected benchmark qubit count".into()); }
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
        "QU_PRS_RUST_NATIVE_MITER case={case} qubits={qubits} gates={} pathvars={paths} phase_terms={}",
        ops.len(), ps.p.terms.len()
    );
    Ok(())
}
