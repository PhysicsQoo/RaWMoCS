//! Cross-repository scientific admission using immutable QuPRS A0 sources.
//! Run only after checking out the pinned A0 source and setting QUPRS_A0_DIR.

use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::{
    solve, solve_with_limits, Gaussian, Limits, Problem, Weights,
};
use serde_json::Value;
use std::path::PathBuf;
use std::str::FromStr;

fn root() -> PathBuf {
    std::env::var_os("QUPRS_A0_DIR")
        .map(PathBuf::from)
        .expect("QUPRS_A0_DIR must point to pinned QuPRS source; fail closed")
}
fn source(name: &str) -> Value {
    let path = root().join(name);
    serde_json::from_slice(&std::fs::read(&path).expect("pinned fixture must exist"))
        .expect("pinned fixture must be valid JSON")
}
fn rational(s: &str) -> BigRational {
    match s.split_once('/') {
        Some((a, b)) => {
            let denominator = BigInt::from_str(b).expect("fraction denominator");
            assert_ne!(denominator, BigInt::from(0), "zero denominator");
            BigRational::new(BigInt::from_str(a).expect("fraction numerator"), denominator)
        }
        None => BigRational::from_integer(BigInt::from_str(s).expect("integer string")),
    }
}
fn gaussian(real: &str, imag: &str) -> Gaussian {
    Gaussian::new(rational(real), rational(imag))
}
fn from_fixture(case: &Value) -> Problem {
    let n = case["nvars"].as_u64().expect("nvars") as usize;
    let clauses = case["clauses"].as_array().expect("clauses").iter()
        .map(|c| c.as_array().expect("clause").iter()
            .map(|v| v.as_i64().expect("literal") as i32).collect())
        .collect();
    let raw = case.get("weights").and_then(Value::as_object);
    let weights = (1..=n).map(|v| {
        let key = v.to_string();
        if let Some(row) = raw.and_then(|r| r.get(&key)) {
            let row = row.as_array().expect("four literal weight components");
            assert_eq!(row.len(), 4);
            let val = |i: usize| row[i].as_str().expect("rational string");
            Weights::new(gaussian(val(0), val(1)), gaussian(val(2), val(3)))
        } else {
            Weights::new(Gaussian::one(), Gaussian::one())
        }
    }).collect();
    Problem::new(n, clauses, weights).expect("valid pinned WMC problem")
}
fn assert_solver(case: &Value, name: &str, expected: &Value) {
    let p = from_fixture(case);
    let value = if expected.get("value").is_some() {
        &expected["value"]
    } else {
        expected
    };
    let want = gaussian(value["re"].as_str().expect("expected re"),
                        value["im"].as_str().expect("expected im"));
    let sat_count = expected["sat_models"].as_u64().expect("expected SAT count");
    let result = solve(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(result.satisfiable, sat_count > 0, "{name}: SAT");
    assert_eq!(result.count, want, "{name}: complex Full WMC");

    // Independently exercise cache-enabled and cache-disabled semantics.
    let no_cache = solve_with_limits(&p, Limits {
        cache_entries: 0,
        ..Limits::default()
    }).expect("same source must solve without cache");
    assert_eq!(result.satisfiable, no_cache.satisfiable, "{name}: cache SAT");
    assert_eq!(result.count, no_cache.count, "{name}: cache count");
    assert_eq!(no_cache.stats.cache_entries, 0, "{name}: cache-disabled");
    assert!(result.stats.cache_entries <= Limits::default().cache_entries);
}

#[test]
#[ignore = "requires pinned QuPRS A0 source checkout"]
fn full_complex_fixtures_reused_from_quprs() {
    let mut full = 0;
    let mut projected = 0;
    for file in ["complex_count_fixtures.json", "quprs_literal_profile_fixtures.json"] {
        let data = source(file);
        assert_eq!(data["schema_version"], 1);
        assert_eq!(data["cases"].as_array().expect("cases").len(), 24);
        for case in data["cases"].as_array().unwrap() {
            match case["mode"].as_str().expect("mode") {
                "wmc" => {
                    full += 1;
                    assert_solver(case, case["id"].as_str().unwrap(), &case["expected"]);
                }
                "pwmc" => projected += 1, // independently checked by original A0 oracle.
                other => panic!("unexpected counting mode: {other}"),
            }
        }
    }
    assert_eq!(full, 39, "source fixture population changed");
    assert_eq!(projected, 9, "projected fixtures must remain oracle-only");
    println!("RaWMoCS SOURCE-PINNED FULL_WMC_SOLVER_PASS=39 PROJECTED_ORACLE_ONLY=9");
}

#[test]
#[ignore = "requires pinned QuPRS A0 source checkout"]
fn original_cache_context_adversaries_reused() {
    let data = source("cache_context_fixtures.json");
    assert_eq!(data["schema_version"], 1);
    let pairs = data["pairs"].as_array().expect("pairs");
    assert_eq!(pairs.len(), 20);
    let mut full = 0;
    let mut projected = 0;
    for pair in pairs {
        let id = pair["id"].as_str().expect("pair ID");
        let mut sides = Vec::new();
        for side in ["left", "right"] {
            let case = &pair[side];
            match case["mode"].as_str().unwrap() {
                "wmc" => {
                    assert_solver(case, &format!("{id}/{side}"), &case["expected"]);
                    let result = solve(&from_fixture(case)).unwrap();
                    sides.push(Some(result));
                    full += 1;
                }
                "epwmc" => {
                    sides.push(None);
                    projected += 1;
                }
                other => panic!("unexpected cache mode: {other}"),
            }
        }
        if let (Some(a), Some(b)) = (&sides[0], &sides[1]) {
            match pair["assert"].as_str().unwrap() {
                "equal" => {
                    assert_eq!(a.count, b.count, "{id}: equivalent contexts");
                    assert_eq!(a.satisfiable, b.satisfiable, "{id}: equivalent SAT");
                }
                "different" => assert_ne!(a.count, b.count, "{id}: distinct contexts"),
                "sat-different" => {
                    assert_eq!(a.count, b.count, "{id}: both count zero");
                    assert_ne!(a.satisfiable, b.satisfiable, "{id}: SAT must differ");
                }
                other => panic!("unknown relation: {other}"),
            }
        }
    }
    assert_eq!(full, 32);
    assert_eq!(projected, 8);
    println!("RaWMoCS SOURCE-PINNED CACHE_CONTEXT_SOLVER_PASS=32 PROJECTED_ORACLE_ONLY=8");
}

#[test]
#[ignore = "requires pinned QuPRS A0 source checkout"]
fn full_wmc_exhaustive_two_variable_reference_universe() {
    // Same enumeration as QuPRS A0f: 256 non-tautological clause subsets ×
    // 256 signed/complex/zero literal weight assignments = 65,536 states.
    const CLAUSES: [&[i32]; 8] = [
        &[1], &[-1], &[2], &[-2], &[1, 2], &[1, -2], &[-1, 2], &[-1, -2],
    ];
    const VALUES: [(i64, i64); 4] = [(0, 0), (1, 0), (-1, 0), (0, 1)];
    let mut checked = 0usize;
    for clause_mask in 0u16..256 {
        let clauses: Vec<Vec<i32>> = CLAUSES.iter().enumerate()
            .filter(|(i, _)| clause_mask & (1 << i) != 0)
            .map(|(_, clause)| clause.to_vec()).collect();
        for weight_mask in 0u16..256 {
            let mut values = Vec::new();
            for j in 0..4 {
                let pair = VALUES[((weight_mask >> (2 * j)) & 3) as usize];
                values.push(Gaussian::integers(pair.0, pair.1));
            }
            let problem = Problem::new(2, clauses.clone(), vec![
                Weights::new(values[0].clone(), values[1].clone()),
                Weights::new(values[2].clone(), values[3].clone()),
            ]).unwrap();
            let got = solve(&problem).expect("A0f two-variable universe");
            let mut expected = Gaussian::zero();
            let mut sat = false;
            for bits in 0..4 {
                let holds = clauses.iter().all(|c| c.iter().any(|&lit| {
                    ((bits >> (lit.unsigned_abs() as usize - 1)) & 1 != 0) == (lit > 0)
                }));
                if holds {
                    sat = true;
                    let term = values[if bits & 1 != 0 { 0 } else { 1 }]
                        .mul(&values[if bits & 2 != 0 { 2 } else { 3 }]);
                    expected = expected.add(&term);
                }
            }
            assert_eq!(got.satisfiable, sat, "mask={clause_mask} weight={weight_mask}");
            assert_eq!(got.count, expected, "mask={clause_mask} weight={weight_mask}");
            checked += 1;
        }
    }
    assert_eq!(checked, 65_536);
    println!("RaWMoCS SOURCE-PINNED A0F_SOLVER_STATES_PASS={checked}");
}
