//! Source-only QuPRS PathSum -> exact Gaussian native-factor bridge.
//! Copy into a fixed QuPRS checkout's examples directory during CI.
//! Never changes QuPRS production code, gates or the original benchmark QASM.
use quprs::{qasm, PathSum, VerificationStrategy};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;

fn source_path(case: &str) -> Option<&'static str> {
    match case {
        "feynman_hwb6" => Some("benchmarks/Feynman/hwb6.qasm"),
        "feynman_qft4" => Some("benchmarks/Feynman/qft_4.qasm"),
        "mqt_qnn_4" => Some("benchmarks/MQTBench/qnn_nativegates_ibm_qiskit_opt0_4.qasm"),
        "mqt_vqe_4" => Some("benchmarks/MQTBench/vqe_nativegates_ibm_qiskit_opt0_4.qasm"),
        "mqt_grover_4" => Some("benchmarks/MQTBench/grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm"),
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let case_os = args.next().ok_or("case required")?;
    let root_os = args.next().ok_or("source root required")?;
    let out_os = args.next().ok_or("output directory required")?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let case = case_os.to_str().ok_or("invalid case encoding")?;
    let source_path = source_path(case).ok_or("unsupported original source")?;
    let source = fs::read_to_string(Path::new(&root_os).join(source_path))?;
    let (ops, qubits) = qasm::parse_qasm_str(&source)?;
    if qubits == 0 || qubits > 12 {
        return Err("qubit dimension out of research range".into());
    }
    let mut base = PathSum::new(qubits);
    base.set_auto_reduce(true);
    let ps = VerificationStrategy::Naive.run(base, &[], &ops);
    if ps.rr_approximate {
        return Err("INELIGIBLE_APPROXIMATE_RR".into());
    }

    let mut ids: BTreeSet<u32> = ps.v.path_vars.iter().copied().collect();
    ids.extend((0..qubits).map(|x| x as u32));
    for monomial in ps.p.terms.keys() {
        ids.extend(monomial.iter().copied());
    }
    for terms in &ps.f.functions {
        for monomial in terms {
            ids.extend(monomial.iter().copied());
        }
    }
    if ids.len() > 48 {
        return Err("INELIGIBLE_EXACT_FACTOR_DIMENSIONS".into());
    }
    let map: BTreeMap<u32, usize> = ids
        .iter()
        .enumerate()
        .map(|(idx, &id)| (id, idx))
        .collect();

    // Whitelist source-exact fourth roots of unity only. An arbitrary Rz
    // (including exact rational radians) must not be silently rounded.
    let mut phases: Vec<(Vec<usize>, u8)> = Vec::new();
    for (monomial, coeff) in &ps.p.terms {
        if coeff.radians.is_some() || !coeff.symbols.is_empty() {
            return Err("INELIGIBLE_NON_GAUSSIAN_PHASE".into());
        }
        let d = i128::from(coeff.constant.denom);
        let n = i128::from(coeff.constant.numer) * 4;
        if d == 0 || n % d != 0 {
            return Err("INELIGIBLE_NON_GAUSSIAN_PHASE".into());
        }
        let k = (n / d).rem_euclid(4) as u8;
        let mut vars: Vec<usize> = monomial.iter().map(|v| map[v]).collect();
        vars.sort_unstable();
        vars.dedup();
        phases.push((vars, k));
    }
    phases.sort();

    let mut outputs: Vec<(usize, Vec<Vec<usize>>)> = Vec::new();
    for (output, terms) in ps.f.functions.iter().enumerate() {
        let mut expressions = Vec::new();
        for term in terms {
            let mut vars: Vec<usize> = term.iter().map(|v| map[v]).collect();
            vars.sort_unstable();
            vars.dedup();
            expressions.push(vars);
        }
        expressions.sort();
        outputs.push((map[&(output as u32)], expressions));
    }
    let mut encoded = format!("p pf {}\n", ids.len());
    for (output, terms) in &outputs {
        encoded.push_str(&format!("o {output} {}\n", terms.len()));
        for vars in terms {
            encoded.push_str(&format!("m {}", vars.len()));
            for &v in vars {
                encoded.push_str(&format!(" {v}"));
            }
            encoded.push('\n');
        }
    }
    for (vars, k) in &phases {
        encoded.push_str(&format!("h {k} {}", vars.len()));
        for &v in vars {
            encoded.push_str(&format!(" {v}"));
        }
        encoded.push('\n');
    }

    // Independent source-level brute-force witness for small exact-Gaussian
    // cases. It directly evaluates ANF and phase indicators, not DP tables.
    if ids.len() > 20 {
        return Err("INELIGIBLE_INDEPENDENT_ORACLE_BUDGET".into());
    }
    let mut sat = false;
    let (mut real, mut imag) = (0i64, 0i64);
    for assignment in 0..(1u64 << ids.len()) {
        let bit = |v: usize| assignment & (1u64 << v) != 0;
        if outputs.iter().any(|(output, terms)| {
            let parity = terms.iter().fold(false, |p, term| {
                p ^ term.iter().all(|&v| bit(v))
            });
            bit(*output) != parity
        }) {
            continue;
        }
        sat = true;
        let degree = phases.iter().fold(0u8, |acc, (vars, k)| {
            if vars.iter().all(|&v| bit(v)) {
                (acc + k) % 4
            } else {
                acc
            }
        });
        match degree {
            0 => real += 1,
            1 => imag += 1,
            2 => real -= 1,
            3 => imag -= 1,
            _ => unreachable!(),
        }
    }
    let folder = Path::new(&out_os);
    fs::create_dir_all(folder)?;
    fs::write(folder.join(format!("{case}-rr-native-factor.txt")), encoded)?;
    fs::write(
        folder.join(format!("{case}-rr-native-factor-oracle.tsv")),
        format!("s {} {real} {imag}\n", usize::from(sat)),
    )?;
    println!(
        "QU_PRS_RR_FACTOR_SOURCE case={case} qasm_ops={} original_paths={} \
         factors={} phase_terms={} exact_gaussian=1 oracle_sat={} oracle_real={real} oracle_imag={imag}",
        ops.len(),
        ids.len(),
        outputs.len() + phases.len(),
        phases.len(),
        usize::from(sat),
    );
    Ok(())
}
