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
const MAX_TRANSFORM_OPS: usize = 250_000_000;

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
    work_ops: usize,
}

fn integer(value: &str) -> Result<usize, String> {
    value.parse().map_err(|_| "ORACLE_MALFORMED_INTEGER".into())
}
fn parse_source(text: &str) -> Result<Input, String> {
    if text.len() > MAX_BYTES {
        return Err("ORACLE_INPUT_BYTE_LIMIT".into());
    }
    let mut rows = text.lines().filter(|line| !line.trim().is_empty());
    let h: Vec<_> = rows
        .next()
        .ok_or("ORACLE_MISSING_HEADER")?
        .split_whitespace()
        .collect();
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
        let head: Vec<_> = rows
            .next()
            .ok_or("ORACLE_TRUNCATED_OUTPUT")?
            .split_whitespace()
            .collect();
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
            let part: Vec<_> = rows
                .next()
                .ok_or("ORACLE_TRUNCATED_TERM")?
                .split_whitespace()
                .collect();
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

// Direct GF(2) Moebius transform, independent of the ROBDD solver.
const LOW_ZETA_MASKS: [u64; 6] = [
    0x5555_5555_5555_5555,
    0x3333_3333_3333_3333,
    0x0f0f_0f0f_0f0f_0f0f,
    0x00ff_00ff_00ff_00ff,
    0x0000_ffff_0000_ffff,
    0x0000_0000_ffff_ffff,
];

fn solve_original_source(input: &Input, limit: usize) -> Result<ResultCount, String> {
    if input.variables > MAX_VARIABLES {
        return Err("ORACLE_VARIABLE_LIMIT".into());
    }
    let word_count = 1usize << input.variables.saturating_sub(6);
    let valid_mask = if input.variables < 6 {
        (1u64 << (1usize << input.variables)) - 1
    } else {
        u64::MAX
    };
    let mut allowed = vec![valid_mask; word_count];
    let mut work_ops = 0usize;
    for equation in &input.outputs {
        let mut values = vec![0u64; word_count];
        for monomial in
            std::iter::once(1u32 << equation.output).chain(equation.terms.iter().copied())
        {
            values[(monomial >> 6) as usize] ^= 1u64 << (monomial & 63);
        }
        for variable in 0..input.variables {
            let step_ops = if variable < 6 {
                word_count
            } else {
                word_count / 2
            };
            work_ops = work_ops
                .checked_add(step_ops)
                .ok_or("ORACLE_WORK_COUNTER_OVERFLOW")?;
            if work_ops > limit {
                return Err("ORACLE_TRANSFORM_WORK_LIMIT".into());
            }
            if variable < 6 {
                let shift = 1usize << variable;
                let filter = LOW_ZETA_MASKS.get(variable).copied().ok_or("ORACLE_LOW_MASK_RANGE")?;
                for word in &mut values {
                    *word ^= (*word & filter) << shift;
                }
            } else {
                let step = 1usize << (variable - 6);
                for base in (0..word_count).step_by(2 * step) {
                    for offset in 0..step {
                        let low = values[base + offset];
                        values[base + step + offset] ^= low;
                    }
                }
            }
        }
        work_ops = work_ops
            .checked_add(word_count)
            .ok_or("ORACLE_WORK_COUNTER_OVERFLOW")?;
        if work_ops > limit {
            return Err("ORACLE_TRANSFORM_WORK_LIMIT".into());
        }
        for (survivor, parity) in allowed.iter_mut().zip(&values) {
            *survivor &= !parity;
        }
    }
    let count = allowed.iter().map(|v| u64::from(v.count_ones())).sum();
    Ok(ResultCount { count, work_ops })
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
    let result = solve_original_source(&input, MAX_TRANSFORM_OPS)?;
    let solve_ns = solve_start.elapsed().as_nanos();
    let header = "status\tvariables\tphase_terms\toutput_equations\tsat\twc_real\twc_imag\ttransform_word_ops\tparse_ns\tsolve_ns\n";
    let line = format!(
        "EXHAUSTIVE_ORIGINAL_ANF_COMPLETED\t{}\t0\t{}\t{}\t{}\t0\t{}\t{}\t{}\n",
        input.variables,
        input.outputs.len(),
        result.count != 0,
        result.count,
        result.work_ops,
        parse_ns,
        solve_ns,
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
                                if (seed * 31 + output * 17 + i * 19 + v * 13) >> (v % 4) & 1 != 0 {
                                    mask |= 1u32 << v;
                                }
                            }
                            mask
                        })
                        .collect();
                    outputs.push(Equation { output, terms });
                }
                let input = Input {
                    variables: n,
                    outputs,
                };
                assert_eq!(
                    solve_original_source(&input, MAX_TRANSFORM_OPS)
                        .unwrap()
                        .count,
                    straightforward(&input),
                    "n={n}, seed={seed}",
                );
            }
        }
    }

    #[test]
    fn exact_source_duplicate_gf2_and_free_variable_semantics() {
        let input =
            parse_source("p anf 6 2 0\no 0 3\nm 1 0\nm 2 2 3\nm 2 2 3\no 1 2\nm 1 2\nm 0\n")
                .unwrap();
        assert_eq!(
            solve_original_source(&input, MAX_TRANSFORM_OPS)
                .unwrap()
                .count,
            32
        );
        let all_free = parse_source("p anf 7 0 0\n").unwrap();
        assert_eq!(
            solve_original_source(&all_free, MAX_TRANSFORM_OPS)
                .unwrap()
                .count,
            128
        );
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
