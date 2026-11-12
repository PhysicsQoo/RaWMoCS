//! Paired native wall-time arithmetic study; isolated from CodSpeed's external installer.
//! Both algorithms use the same source-locked inputs, process and compiler.
//! Not a production performance guarantee or a memory-allocation measurement.
use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::Gaussian;
use std::hint::black_box;
use std::time::Instant;

fn r(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn reference_add(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(&a.real + &b.real, &a.imag + &b.imag)
}
fn run(name: &str, left: &Gaussian, right: &Gaussian, rounds: usize) {
    let expected = reference_add(left, right);
    assert_eq!(left.add(right), expected, "exact oracle mismatch: {name}");
    let count = 4096_u64;
    for round in 0..rounds {
        for trial in 0..2 {
            let candidate = (round + trial) % 2 == 0;
            let label = if candidate { "candidate" } else { "reference" };
            let started = Instant::now();
            for _ in 0..count {
                if candidate {
                    black_box(black_box(left).add(black_box(right)));
                } else {
                    black_box(reference_add(black_box(left), black_box(right)));
                }
            }
            let total_ns = started.elapsed().as_nanos();
            assert!(total_ns > 0, "clock gave zero nanoseconds");
            println!("{name},{round},{label},{count},{total_ns}");
        }
    }
}

fn main() {
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        vec!["--paired".to_string()],
        "only predeclared paired run is supported"
    );
    let z = Gaussian::zero();
    let a = Gaussian::new(r(-11, 19), r(23, 29));
    let b = Gaussian::new(r(31, 37), r(-41, 43));
    let numerator: BigInt = (BigInt::from(1_u8) << 521_usize) + 12345;
    let denominator: BigInt = (BigInt::from(1_u8) << 607_usize) + 98765;
    let large = Gaussian::new(
        BigRational::new(numerator.clone(), denominator.clone()),
        BigRational::new(-denominator, numerator),
    );
    println!("scenario,round,implementation,iterations,total_wall_ns");
    run("left_zero", &z, &a, 7);
    run("right_zero", &a, &z, 7);
    run("both_zero", &z, &z, 7);
    run("dense", &a, &b, 7);
    run("left_zero_large", &z, &large, 7);
    run("dense_large", &large, &b, 7);
}
