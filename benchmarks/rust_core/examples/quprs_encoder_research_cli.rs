//! Research-only finite-printed-decimal GPMC-style weighted-CNF solver adapter.
//! Not QuPRS's production API or a claim of source-phase exactness.
//! Bounded structural parsing; solver budgets remain default and explicit.
use rawmocs_benchmarks::quprs_printed_cnf::printed_decimal;
use rawmocs_core::{solve_with_limits, Gaussian, Limits, Problem, Weights};
use std::path::Path;

fn parse_emitted(source: &str) -> Result<Problem, String> {
    if source.is_empty() || source.len() > 2_000_000 {
        return Err("source too large or empty".into());
    }
    let mut lines = source.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().ok_or("missing DIMACS header")?;
    let h: Vec<&str> = header.split_whitespace().collect();
    if h.len() != 4 || h[0] != "p" || h[1] != "cnf" {
        return Err("unsupported DIMACS header".into());
    }
    let n = h[2]
        .parse::<usize>()
        .map_err(|_| "invalid variable count")?;
    let num_clauses = h[3].parse::<usize>().map_err(|_| "invalid clause count")?;
    if !(1..=4096).contains(&n) || num_clauses > 50_000 {
        return Err("unadmitted research CNF dimensions".into());
    }
    if lines.next().map(str::trim) != Some("c t wmc") {
        return Err("missing QuPRS weighted marker".into());
    }
    let mut weights = vec![Weights::new(Gaussian::one(), Gaussian::one()); n];
    let mut used = vec![[false; 2]; n];
    let mut clauses = Vec::with_capacity(num_clauses);
    let mut clause_started = false;
    for line in lines {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.first() == Some(&"c") {
            if clause_started
                || fields.len() != 7
                || fields[1] != "p"
                || fields[2] != "weight"
                || fields[6] != "0"
            {
                return Err("unexpected weight syntax or order".into());
            }
            let literal = fields[3]
                .parse::<i32>()
                .map_err(|_| "invalid weight literal")?;
            let index = literal.unsigned_abs() as usize;
            if !(1..=n).contains(&index) {
                return Err("weight outside variable range".into());
            }
            let side = usize::from(literal < 0);
            if used[index - 1][side] {
                return Err("duplicate source literal weight".into());
            }
            used[index - 1][side] = true;
            let weight = Gaussian::new(printed_decimal(fields[4])?, printed_decimal(fields[5])?);
            if side == 0 {
                weights[index - 1].positive = weight;
            } else {
                weights[index - 1].negative = weight;
            }
        } else {
            clause_started = true;
            if fields.last() != Some(&"0") {
                return Err("non-terminated clause".into());
            }
            let mut clause = Vec::with_capacity(fields.len() - 1);
            for token in &fields[..fields.len() - 1] {
                let lit = token.parse::<i32>().map_err(|_| "invalid clause literal")?;
                if lit == 0 || lit.unsigned_abs() as usize > n {
                    return Err("out-of-range source literal".into());
                }
                clause.push(lit);
            }
            clauses.push(clause);
            if clauses.len() > num_clauses {
                return Err("excess source clauses".into());
            }
        }
    }
    if clauses.len() != num_clauses {
        return Err("missing source clauses".into());
    }
    Problem::new(n, clauses, weights).map_err(|err| format!("invalid weighted CNF: {err}"))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let filename = args
        .next()
        .ok_or("usage: quprs_encoder_research_cli FILE.cnf")?;
    if args.next().is_some() {
        return Err("expected exactly one source CNF".into());
    }
    let input = std::fs::read_to_string(Path::new(&filename))
        .map_err(|err| format!("CNF input read failure: {err}"))?;
    let problem = parse_emitted(&input)?;
    let report = solve_with_limits(&problem, Limits::default())
        .map_err(|err| format!("bounded exact source solver returned UNKNOWN: {err}"))?;
    println!(
        "s {}",
        if report.satisfiable {
            "SATISFIABLE"
        } else {
            "UNSATISFIABLE"
        }
    );
    println!("c r rational {} {}", report.count.real, report.count.imag);
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("RESEARCH_SOLVER_UNQUALIFIED: {err}");
        std::process::exit(2);
    }
}
