//! Independent exhaustive source-ANF bitset oracle for phase-free, unit-weight
//! PathSum traces. No BDD, no polynomial DAG, and no third-party WMC search.
//! Enumerates ALL original assignments in 64-way packed Boolean truth tables.
//! Research-only and fail-closed at 24 original variables and work budget.
use std::env;
use std::fs;
use std::time::Instant;

const MAX_VARIABLES: usize = 24;
const MAX_TERMS: usize = 200_000;
const MAX_BYTES: usize = 8_000_000;
const MAX_TOGGLE_OPS: usize = 400_000_000;

struct Equation {
    output: usize,
    terms: Vec<u32>,
}
struct Input {
    variables: usize,
    outputs: Vec<Equation>,
}
struct ResultCount {
    count: u64,
    toggles: usize,
}

fn integer(value: &str) -> Result<usize, String> {
    value.parse().map_err(|_| "ORACLE_MALFORMED_INTEGER".into())
}
fn parse_source(text: &str) -> Result<Input, String> {
    if text.len() > MAX_BYTES {
        return Err("ORACLE_INPUT_BYTE_LIMIT".into());
    }
    let mut rows = text.lines().filter(|line| !line.trim().is_empty());
    let h: Vec<_> = rows.next().ok_or("ORACLE_MISSING_HEADER")?
        .split_whitespace().collect();
    if h.len() != 5 || h[0] != "p" || h[1] != "anf" {
        return Err("ORACLE_INVALID_HEADER".into());
    }
    let variables = integer(h[2])?;
    let num_eqs = integer(h[3])?;
    let phase = integer(h[4])?;
    if variables > MAX_VARIABLES || num_eqs > 256 {
        return Err("ORACLE_VARIABLE_OR_OUTPUT_LIMIT".into());
    }
    if phase != 0 {
        return Err("ORACLE_COMPLEX_PHASE_INELIGIBLE".into());
    }
    let mut outputs = Vec::new();
    let mut terms_seen = 0;
    for _ in 0..num_eqs {
        let head: Vec<_> = rows.next().ok_or("ORACLE_TRUNCATED_OUTPUT")?
            .split_whitespace().collect();
        if head.len() != 3 || head[0] != "o" {
            return Err("ORACLE_INVALID_OUTPUT".into());
        }
        let output = integer(head[1])?;
        let count = integer(head[2])?;
        terms_seen += count;
        if output >= variables || terms_seen > MAX_TERMS {
            return Err("ORACLE_OUTPUT_OR_TERM_LIMIT".into());
        }
        let mut terms = Vec::with_capacity(count);
        for _ in 0..count {
            let part: Vec<_> = rows.next().ok_or("ORACLE_TRUNCATED_TERM")?
                .split_whitespace().collect();
            if part.len() < 2 || part[0] != "m" {
                return Err("ORACLE_INVALID_TERM".into());
            }
            let degree = integer(part[1])?;
            if degree > variables || part.len() != degree + 2 {
                return Err("ORACLE_INVALID_DEGREE".into());
            }
            let mut mask = 0u32;
            for &v in &part[2..] {
                let variable = integer(v)?;
                if variable >= variables {
                    return Err("ORACLE_TERM_VARIABLE_RANGE".into());
                }
                mask |= 1u32 << variable;
            }
            terms.push(mask);
        }
        outputs.push(Equation { output, terms });
    }
    if rows.next().is_some() {
        return Err("ORACLE_TRAILING_DATA".into());
    }
    Ok(Input { variables, outputs })
}

fn low_patterns() -> [u64; 64] {
    let mut patterns = [0u64; 64];
    for mask in 0..64usize {
        for a in 0..64 {
            if a & mask == mask {
                patterns[mask] |= 1u64 << a;
            }
        }
    }
    patterns
}

fn toggle_term(
    table: &mut [u64],
    mask: u32,
    patterns: &[u64; 64],
    high_variables: usize,
    visited: &mut usize,
    limit: usize,
) -> Result<(), String> {
    let required_high = (mask >> 6) as usize;
    let high_full = (1usize << high_variables) - 1;
    let free = high_full ^ required_high;
    let operations = 1usize << free.count_ones();
    *visited = visited.checked_add(operations)
        .ok_or("ORACLE_TOGGLE_COUNTER_OVERFLOW")?;
    if *visited > limit {
        return Err("ORACLE_WORK_LIMIT".into());
    }
    let bits = patterns[(mask & 63) as usize];
    let mut optional = free;
    loop {
        table[optional | required_high] ^= bits;
        if optional == 0 {
            break;
        }
        optional = (optional - 1) & free;
    }
    Ok(())
}

fn solve_original_source(input: &Input, limit: usize) -> Result<ResultCount, String> {
    if input.variables > MAX_VARIABLES {
        return Err("ORACLE_VARIABLE_LIMIT".into());
    }
    let high_vars = input.variables.saturating_sub(6);
    let chunks = 1usize << high_vars;
    let valid = if input.variables < 6 {
        (1u64 << (1usize << input.variables)) - 1
    } else {
        u64::MAX
    };
    let patterns = low_patterns();
    let mut survivors = vec![valid; chunks];
    let mut toggles = 0usize;
    for eq in &input.outputs {
        let mut polynomial = vec![0u64; chunks];
        // Direct evaluation of the ORIGINAL source equation: the output
        // literal must be XORed with its ANF right-hand side.
        toggle_term(
            &mut polynomial,
            1u32 << eq.output,
            &patterns,
            high_vars,
            &mut toggles,
            limit,
        )?;
        for &term in &eq.terms {
            toggle_term(
                &mut polynomial,
                term,
                &patterns,
                high_vars,
                &mut toggles,
                limit,
            )?;
        }
        for (state, parity) in survivors.iter_mut().zip(polynomial.iter()) {
            *state &= !parity;
        }
    }
    let count = survivors.iter().map(|v| u64::from(v.count_ones())).sum();
    Ok(ResultCount { count, toggles })
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let source = args.next().ok_or("usage: SOURCE.anf [RESULT.tsv]")?;
    let destination = args.next();
    if args.next().is_some() {
        return Err("ORACLE_TOO_MANY_ARGUMENTS".into());
    }
    let start = Instant::now();
    let input_text = fs::read_to_string(source).map_err(|e| e.to_string())?;
    let input = parse_source(&input_text)?;
    let parse_ns = start.elapsed().as_nanos();
    let solve_start = Instant::now();
    let result = solve_original_source(&input, MAX_TOGGLE_OPS)?;
    let solve_ns = solve_start.elapsed().as_nanos();
    let header = "status\tvariables\tphase_terms\toutput_equations\tsat\twc_real\twc_imag\ttoggle_ops\tparse_ns\tsolve_ns\n";
    let line = format!(
        "EXHAUSTIVE_ORIGINAL_ANF_COMPLETED\t{}\t0\t{}\t{}\t{}\t0\t{}\t{}\t{}\n",
        input.variables, input.outputs.len(), result.count != 0,
        result.count, result.toggles, parse_ns, solve_ns,
    );
    if let Some(path) = destination {
        fs::write(path, format!("{header}{line}")).map_err(|e| e.to_string())?;
    }
    print!("{header}{line}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("EXHAUSTIVE_SOURCE_ORACLE_CENSORED {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn straightforward(input: &Input) -> u64 {
        let mut count = 0u64;
        for assignment in 0..(1u64 << input.variables) {
            let accepted = input.outputs.iter().all(|eq| {
                let mut parity = (assignment & (1u64 << eq.output)) != 0;
                for &term in &eq.terms {
                    parity ^= ((assignment as u32) & term) == term;
                }
                !parity
            });
            count += u64::from(accepted);
        }
        count
    }

    #[test]
    fn independent_bitparallel_matches_scalar_complete_enumeration() {
        for n in 1..=9usize {
            for seed in 0..20usize {
                let mut outputs = Vec::new();
                for output in 0..n.min(4) {
                    let terms = (0..14usize)
                        .map(|i| {
                            let mut mask = 0u32;
                            for v in 0..n {
                                if (seed * 31 + output * 17 + i * 19 + v * 13)
                                    >> (v % 4) & 1 != 0
                                {
                                    mask |= 1u32 << v;
                                }
                            }
                            mask
                        })
                        .collect();
                    outputs.push(Equation { output, terms });
                }
                let input = Input { variables: n, outputs };
                assert_eq!(
                    solve_original_source(&input, MAX_TOGGLE_OPS).unwrap().count,
                    straightforward(&input),
                    "n={n}, seed={seed}",
                );
            }
        }
    }

    #[test]
    fn exact_source_duplicate_gf2_and_free_variable_semantics() {
        let input = parse_source(
            "p anf 6 2 0\no 0 3\nm 1 0\nm 2 2 3\nm 2 2 3\no 1 2\nm 1 2\nm 0\n",
        ).unwrap();
        assert_eq!(solve_original_source(&input, MAX_TOGGLE_OPS).unwrap().count, 32);
        let all_free = parse_source("p anf 7 0 0\n").unwrap();
        assert_eq!(solve_original_source(&all_free, MAX_TOGGLE_OPS).unwrap().count, 128);
    }

    #[test]
    fn source_and_work_budget_fail_closed() {
        for bad in [
            "p anf 25 0 0\n",
            "p anf 4 0 1\n",
            "p anf 3 1 0\no 0 1\nm 2 0\n",
            "p anf 3 1 0\no 0 1\nm 1 4\n",
            "p anf 2 0 0\nm 1 1\n",
        ] {
            assert!(parse_source(bad).is_err());
        }
        let input = parse_source("p anf 20 1 0\no 0 1\nm 0\n").unwrap();
        assert!(solve_original_source(&input, 1).is_err());
    }
}
