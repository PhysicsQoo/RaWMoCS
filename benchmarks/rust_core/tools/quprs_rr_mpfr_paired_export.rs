//! Original QuPRS Path-sum -> TWO same-source, same-printed-phase MPFR inputs.
//! This file is copied to an isolated pinned QuPRS checkout by research CI.
//! It does not change QuPRS, GPMC, or any released RaWMoCS solver.
use quprs::{qasm, wmc::WmcManager, PathSum, VerificationStrategy};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn source_path(case: &str) -> Option<&'static str> {
    match case {
        "feynman_hwb6" => Some("benchmarks/Feynman/hwb6.qasm"),
        "feynman_qft4" => Some("benchmarks/Feynman/qft_4.qasm"),
        "feynman_grover5" => Some("benchmarks/Feynman/grover_5.qasm"),
        "feynman_barenco5" => Some("benchmarks/Feynman/barenco_tof_5.qasm"),
        "feynman_modmult55" => Some("benchmarks/Feynman/mod_mult_55.qasm"),
        "feynman_barenco10" => Some("benchmarks/Feynman/barenco_tof_10.qasm"),
        "mqt_qnn_4" => Some("benchmarks/MQTBench/qnn_nativegates_ibm_qiskit_opt0_4.qasm"),
        "mqt_vqe_4" => Some("benchmarks/MQTBench/vqe_nativegates_ibm_qiskit_opt0_4.qasm"),
        "mqt_grover_4" => {
            Some("benchmarks/MQTBench/grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm")
        }
        _ => None,
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let case_os = args.next().ok_or("case required")?;
    let root = args.next().ok_or("QuPRS root required")?;
    let output = args.next().ok_or("output directory required")?;
    let precision = args.next().ok_or("MPFR bit width required")?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let case = case_os.to_str().ok_or("non-UTF8 case")?;
    let bits: usize = precision.to_str().ok_or("non-UTF8 precision")?.parse()?;
    if !matches!(bits, 100 | 333) {
        return Err("research requires exactly 100 or 333 MPFR bits".into());
    }
    let source = fs::read_to_string(
        Path::new(&root).join(source_path(case).ok_or("unsupported original QASM source")?),
    )?;
    let clock = Instant::now();
    let (ops, qubits) = qasm::parse_qasm_str(&source)?;
    // Wide source is admitted only through the unchanged, bounded research
    // oracle / native-factor gates; production verification is unaffected.
    if qubits == 0 || qubits > 24 {
        return Err("invalid QuPRS original qubit dimension".into());
    }
    let mut base = PathSum::new(qubits);
    base.set_auto_reduce(true);
    let ps = VerificationStrategy::Naive.run(base, &[], &ops);
    let quprs_rr_ns = clock.elapsed().as_nanos();
    if ps.rr_approximate {
        return Err("source reduction used an approximate rule".into());
    }

    // Use THE SAME source-precision WmcManager for GPMC CNF and native
    // factor weights; no independent f64 phase conversion is permitted.
    let encode_start = Instant::now();
    let mut manager = WmcManager::new(&ps);
    manager.enable_precise_gpmc_weights(bits);
    manager.encode_trace(&ps)?;
    let dimacs = manager.to_dimacs_string("gpmc");
    let cnf_encode_ns = encode_start.elapsed().as_nanos();
    let mut positive_weights: Vec<(i32, String, String)> = Vec::new();
    for line in dimacs.lines() {
        let cols: Vec<_> = line.split_whitespace().collect();
        if cols.len() == 7 && cols[0..3] == ["c", "p", "weight"] {
            let literal: i32 = cols[3].parse()?;
            if literal > 0 {
                positive_weights.push((literal, cols[4].to_owned(), cols[5].to_owned()));
            }
        }
    }
    positive_weights.sort_unstable_by_key(|w| w.0);
    if positive_weights.len() != ps.p.terms.len() {
        return Err("phase weight cardinality mismatch with original QuPRS".into());
    }

    let mut ids: BTreeSet<u32> = ps.v.path_vars.iter().copied().collect();
    ids.extend((0..qubits).map(|v| v as u32));
    for monomial in ps.p.terms.keys() {
        ids.extend(monomial.iter().copied());
    }
    for equation in &ps.f.functions {
        for term in equation {
            ids.extend(term.iter().copied());
        }
    }
    if ids.len() > 256 {
        return Err("native source variable limit exceeded".into());
    }
    let map: BTreeMap<u32, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, &source)| (source, i))
        .collect();
    let mut factor = format!("p pf {}\n", ids.len());
    for (output, equation) in ps.f.functions.iter().enumerate() {
        let mut terms: Vec<Vec<usize>> = equation
            .iter()
            .map(|term| {
                let mut vars: Vec<_> = term.iter().map(|v| map[v]).collect();
                vars.sort_unstable();
                vars.dedup();
                vars
            })
            .collect();
        terms.sort();
        factor.push_str(&format!("o {} {}\n", map[&(output as u32)], terms.len()));
        for vars in terms {
            factor.push_str(&format!("m {}", vars.len()));
            for var in vars {
                factor.push_str(&format!(" {var}"));
            }
            factor.push('\n');
        }
    }

    // WmcManager::encode_phase_polynomial creates one positive-weight z
    // BEFORE any monomial auxiliary path-variable allocation, in exactly
    // ps.p.terms iteration order. Thus increasing z literal ID is the
    // original traversal order. Recheck phase count and monotonicity.
    let mut previous = 0i32;
    for ((mono, coeff), (z, re, im)) in ps.p.terms.iter().zip(positive_weights.iter()) {
        if *z <= previous || !coeff.symbols.is_empty() {
            return Err("invalid phase order or unbound source symbol".into());
        }
        previous = *z;
        let mut vars: Vec<_> = mono.iter().map(|v| map[v]).collect();
        vars.sort_unstable();
        vars.dedup();
        factor.push_str(&format!("h {re} {im} {}", vars.len()));
        for var in vars {
            factor.push_str(&format!(" {var}"));
        }
        factor.push('\n');
    }
    let total_prepare_ns = clock.elapsed().as_nanos();
    let folder = Path::new(&output);
    fs::create_dir_all(folder)?;
    fs::write(folder.join(format!("{case}-{bits}-rr-gpmc.cnf")), dimacs)?;
    fs::write(folder.join(format!("{case}-{bits}-rr-factor.pf")), factor)?;
    println!(
        "RR_PRECISION_PAIR case={case} bits={bits} source_qubits={qubits} \
         original_gates={} residual_vars={} phase_terms={} \
         rr_ns={quprs_rr_ns} cnf_encode_ns={cnf_encode_ns} \
         prepare_ns={total_prepare_ns}",
        ops.len(),
        ids.len(),
        positive_weights.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("RR_PRECISION_PAIR_UNQUALIFIED {error}");
        std::process::exit(2);
    }
}
