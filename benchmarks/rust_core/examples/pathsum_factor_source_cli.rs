//! Opt-in independent native-factor CLI for source-locked QuPRS RR export.
//! A trusted source witness is rechecked; this does not support arbitrary
//! complex angles, general CNF or production solver selection.
use rawmocs_benchmarks::pathsum_factor_research::{
    FactorLimits, OutputEquality, PathsumFactors, PhaseMonomial,
};
use rawmocs_core::{Gaussian, Weights};
use std::env;
use std::fs;
use std::time::Instant;

fn number(s: &str) -> Result<usize, String> {
    s.parse::<usize>()
        .map_err(|_| "invalid factor file integer".into())
}

fn parsed_model(text: &str) -> Result<PathsumFactors, String> {
    if text.len() > 2_000_000 {
        return Err("native factor document exceeds source budget".into());
    }
    let lines: Vec<&str> = text.lines().filter(|s| !s.trim().is_empty()).collect();
    if lines.is_empty() {
        return Err("native factor document is empty".into());
    }
    let header: Vec<&str> = lines[0].split_whitespace().collect();
    if header.len() != 3 || header[..2] != ["p", "pf"] {
        return Err("invalid native factor header".into());
    }
    let n = number(header[2])?;
    if n > 48 {
        return Err("native factor variable budget exceeded".into());
    }
    let mut problem = PathsumFactors {
        variables: n,
        weights: vec![Weights::new(Gaussian::one(), Gaussian::one()); n],
        parity: vec![],
        phase: vec![],
        outputs: vec![],
        scalar: Gaussian::one(),
    };
    let mut i = 1usize;
    while i < lines.len() {
        let fields: Vec<&str> = lines[i].split_whitespace().collect();
        if fields.is_empty() {
            return Err("invalid empty factor row".into());
        }
        match fields[0] {
            "o" => {
                if fields.len() != 3 {
                    return Err("invalid output equation".into());
                }
                let output = number(fields[1])?;
                let terms = number(fields[2])?;
                if terms > 100_000 || terms > lines.len().saturating_sub(i + 1) {
                    return Err("truncated or oversized output polynomial".into());
                }
                let mut monomials = Vec::with_capacity(terms);
                for _ in 0..terms {
                    i += 1;
                    let f: Vec<&str> = lines[i].split_whitespace().collect();
                    if f.len() < 2 || f[0] != "m" {
                        return Err("missing ANF monomial".into());
                    }
                    let size = number(f[1])?;
                    if size > n || size + 2 != f.len() {
                        return Err("invalid ANF monomial size".into());
                    }
                    monomials.push(
                        f[2..]
                            .iter()
                            .map(|v| number(v))
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                }
                problem.outputs.push(OutputEquality {
                    output,
                    constant: false,
                    terms: monomials,
                });
            }
            "h" => {
                if fields.len() < 3 {
                    return Err("invalid phase factor header".into());
                }
                let phase = number(fields[1])?;
                let size = number(fields[2])?;
                if phase >= 4 || size > n || size + 3 != fields.len() {
                    return Err("unadmitted phase or malformed monomial".into());
                }
                let weight_if_true = match phase {
                    0 => Gaussian::one(),
                    1 => Gaussian::integers(0, 1),
                    2 => Gaussian::integers(-1, 0),
                    3 => Gaussian::integers(0, -1),
                    _ => unreachable!(),
                };
                let variables = fields[3..]
                    .iter()
                    .map(|v| number(v))
                    .collect::<Result<Vec<_>, _>>()?;
                problem.phase.push(PhaseMonomial {
                    variables,
                    weight_if_true,
                });
            }
            _ => return Err("unknown native factor tag".into()),
        }
        i += 1;
    }
    problem.validate()?;
    Ok(problem)
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let first = args.next().ok_or("usage: [--monotone-ice] FACTORS ORACLE")?;
    let monotone_ice = first.to_str() == Some("--monotone-ice");
    let model_path = if monotone_ice {
        args.next().ok_or("missing monotone source factors")?
    } else {
        first
    };
    let oracle_path = args.next().ok_or("missing exact source oracle")?;
    if args.next().is_some() {
        return Err("unexpected argument after oracle".into());
    }
    let started = Instant::now();
    let model = fs::read_to_string(model_path).map_err(|e| e.to_string())?;
    let problem = parsed_model(&model)?;
    let parsing_ns = started.elapsed().as_nanos();
    let profile_start = Instant::now();
    let profile = problem.profile()?;
    let profile_ns = profile_start.elapsed().as_nanos();
    if monotone_ice {
        let start = Instant::now();
        let outcome = problem.count_monotone_trace_bounded()?;
        let solve_ns = start.elapsed().as_nanos();
        let reference = fs::read_to_string(oracle_path).map_err(|e| e.to_string())?;
        let expected: Vec<&str> = reference.split_whitespace().collect();
        if expected.len() != 4 || expected[0] != "s" {
            return Err("invalid monotone source oracle".into());
        }
        let expected_sat = match expected[1] {
            "0" => false,
            "1" => true,
            _ => return Err("invalid monotone oracle SAT flag".into()),
        };
        let re = expected[2]
            .parse::<i64>()
            .map_err(|_| "invalid oracle real")?;
        let im = expected[3]
            .parse::<i64>()
            .map_err(|_| "invalid oracle imaginary")?;
        if outcome.satisfiable != expected_sat
            || outcome.count != Gaussian::integers(re, im)
        {
            return Err(format!(
                "SOURCE_MONOTONE_ORACLE_MISMATCH sat={} count=({}, {})",
                outcome.satisfiable, outcome.count.real, outcome.count.imag
            ));
        }
        println!(
            "QU_PRS_RR_MONOTONE_MATCH vars={} constraints={} subsets={} \
             sat={} real={} imag={} parse_ns={} profile_ns={} solve_ns={}",
            problem.variables,
            outcome.forbidden_conjunctions,
            outcome.subsets_evaluated,
            outcome.satisfiable,
            outcome.count.real,
            outcome.count.imag,
            parsing_ns,
            profile_ns,
            solve_ns
        );
        return Ok(());
    }
    let start = Instant::now();
    let result = problem.solve_bounded(FactorLimits::default())?;
    let solve_ns = start.elapsed().as_nanos();
    let reference = fs::read_to_string(oracle_path).map_err(|e| e.to_string())?;
    let expected: Vec<&str> = reference.split_whitespace().collect();
    if expected.len() != 4 || expected[0] != "s" {
        return Err("invalid independent source oracle".into());
    }
    let expected_sat = match expected[1] {
        "0" => false,
        "1" => true,
        _ => return Err("invalid oracle SAT flag".into()),
    };
    let re = expected[2]
        .parse::<i64>()
        .map_err(|_| "invalid oracle real")?;
    let im = expected[3]
        .parse::<i64>()
        .map_err(|_| "invalid oracle imaginary")?;
    if result.satisfiable != expected_sat || result.count != Gaussian::integers(re, im) {
        return Err(format!(
            "SOURCE_ORACLE_MISMATCH sat={} count=({}, {})",
            result.satisfiable, result.count.real, result.count.imag
        ));
    }
    println!(
        "QU_PRS_RR_FACTOR_MATCH vars={} phase_terms={} xor_output={} \
         native_factor_scope={} primal_minfill_width_upper={} \
         sat={} real={} imag={} initial_factors={} peak_scope={} visited_cells={} \
         parse_ns={} profile_ns={} solve_ns={}",
        problem.variables,
        problem.phase.len(),
        problem.outputs.len(),
        profile.max_constraint_scope,
        profile.primal_minfill_width_upper,
        result.satisfiable,
        result.count.real,
        result.count.imag,
        result.stats.input_factors,
        result.stats.peak_scope,
        result.stats.visited_cells,
        parsing_ns,
        profile_ns,
        solve_ns
    );
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("QU_PRS_RR_FACTOR_UNQUALIFIED: {err}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_native_pathsum_roundtrip() {
        let text = "p pf 3\no 2 2\nm 2 0 1\nm 0\nh 1 1 0\n";
        let p = parsed_model(text).unwrap();
        assert_eq!(p.variables, 3);
        assert_eq!(p.phase.len(), 1);
        assert_eq!(p.outputs.len(), 1);
        let r = p.solve_bounded(FactorLimits::default()).unwrap();
        assert!(r.satisfiable);
    }

    #[test]
    fn rejects_incomplete_monomial_and_phase() {
        assert!(parsed_model("p pf 1\no 0 2\nm 1 0\n").is_err());
        assert!(parsed_model("p pf 2\nh 4 0\n").is_err());
        assert!(parsed_model("p pf 2\nh 1 2 1 9\n").is_err());
        assert!(parsed_model("p pf 2\nh 1 0\nunknown 0\n").is_err());
    }
}
