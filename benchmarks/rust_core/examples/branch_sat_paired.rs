//! Fixed, full complex WMC SAT-branch holdout for pinned-baseline paired CI.
//! One process per case; fixture and exact analytical oracle are outside timing.
use rawmocs_core::{solve_with_limits, Gaussian, Limits, Problem, Weights};
use std::time::Instant;

fn reference_mul(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

fn input(family: &str, blocks: usize) -> (Problem, bool, Gaussian) {
    assert!([8, 16, 32].contains(&blocks), "fixed block cohort only");
    assert!(
        ["one", "three", "cancel", "unsat"].contains(&family),
        "fixed SAT families only"
    );
    let mut clauses = Vec::with_capacity(4 * blocks);
    let mut weights = Vec::with_capacity(2 * blocks);
    for i in 0..blocks {
        let x = (2 * i + 1) as i32;
        let y = x + 1;
        clauses.push(vec![x, y]);
        if family == "one" || family == "unsat" {
            clauses.push(vec![x, -y]);
            clauses.push(vec![-x, y]);
        }
        if family == "unsat" {
            clauses.push(vec![-x, -y]);
        }
        if family == "cancel" {
            weights.push(Weights::new(
                Gaussian::integers(1, 0),
                Gaussian::integers(-1, 0),
            ));
            weights.push(Weights::new(
                Gaussian::integers(1, 0),
                Gaussian::zero(),
            ));
        } else {
            weights.push(Weights::new(
                Gaussian::integers(2, 1),
                Gaussian::integers(-3, 4),
            ));
            weights.push(Weights::new(
                Gaussian::integers(1, -2),
                Gaussian::integers(5, 3),
            ));
        }
    }
    let p = Problem::new(2 * blocks, clauses, weights).expect("fixed valid CNF");
    let (sat, single_block) = match family {
        "one" => (true, Gaussian::integers(4, -3)),
        "three" => (true, Gaussian::integers(16, 18)),
        "cancel" => (true, Gaussian::zero()),
        "unsat" => (false, Gaussian::zero()),
        _ => unreachable!(),
    };
    let mut oracle = Gaussian::one();
    for _ in 0..blocks {
        oracle = reference_mul(&oracle, &single_block);
    }
    (p, sat, oracle)
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 4, "usage: --case FAMILY BLOCKS CACHE_CAP");
    assert_eq!(args[0], "--case", "only predeclared cases allowed");
    let family = args[1].as_str();
    let blocks = args[2].parse::<usize>().expect("blocks");
    let cache_cap = args[3].parse::<usize>().expect("cache cap");
    assert!([0_usize, 10_000].contains(&cache_cap));
    let (p, sat, oracle) = input(family, blocks);
    let started = Instant::now();
    let result = solve_with_limits(
        &p,
        Limits {
            cache_entries: cache_cap,
            ..Limits::default()
        },
    )
    .expect("fixed bounded WMC case");
    let elapsed_ns = started.elapsed().as_nanos();
    assert!(elapsed_ns > 0);
    assert_eq!(result.satisfiable, sat, "independent boolean SAT control");
    assert_eq!(result.count, oracle, "independent exact complex WMC control");
    println!(
        "{family},{blocks},{cache_cap},{elapsed_ns},{sat},{},{},{},{},{},{}",
        result.count.real,
        result.count.imag,
        result.stats.nodes,
        result.stats.cache_entries,
        result.stats.cache_hits,
        result.stats.decompositions
    );
}
