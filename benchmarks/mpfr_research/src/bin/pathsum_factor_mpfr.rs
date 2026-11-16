//! Exact-source-*printed* MPFR native-factor sum-product research.
//! No GPMC search/heuristic reuse; factors are original QuPRS ANF and phases.
//! This CLI consumes the SAME printed MPFR weights as GPMC's post-RR CNF.
//! It is not a production solver and cannot certify transcendental phases.
use rug::Float;
use std::env;
use std::fs;
use std::time::Instant;

#[derive(Clone, Debug)]
struct Complex {
    re: Float,
    im: Float,
}
impl Complex {
    fn zero(bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, 0),
            im: Float::with_val(bits, 0),
        }
    }
    fn one(bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, 1),
            im: Float::with_val(bits, 0),
        }
    }
    fn add(&self, rhs: &Self, bits: u32) -> Self {
        Self {
            re: Float::with_val(bits, &self.re + &rhs.re),
            im: Float::with_val(bits, &self.im + &rhs.im),
        }
    }
    fn mul(&self, rhs: &Self, bits: u32) -> Self {
        // Match research GPMC arithmetic width: four separately rounded
        // products, then rounded real and imaginary additions.
        let arbr = Float::with_val(bits, &self.re * &rhs.re);
        let aibi = Float::with_val(bits, &self.im * &rhs.im);
        let arbi = Float::with_val(bits, &self.re * &rhs.im);
        let aibr = Float::with_val(bits, &self.im * &rhs.re);
        Self {
            re: Float::with_val(bits, &arbr - &aibi),
            im: Float::with_val(bits, &arbi + &aibr),
        }
    }
}
#[derive(Clone)]
struct Cell {
    sat: bool,
    value: Complex,
}
impl Cell {
    fn zero(bits: u32) -> Self {
        Self {
            sat: false,
            value: Complex::zero(bits),
        }
    }
    fn one(bits: u32) -> Self {
        Self {
            sat: true,
            value: Complex::one(bits),
        }
    }
    fn product(&self, rhs: &Self, bits: u32) -> Self {
        if !self.sat || !rhs.sat {
            return Self::zero(bits);
        }
        Self {
            sat: true,
            value: self.value.mul(&rhs.value, bits),
        }
    }
    fn sum(&self, rhs: &Self, bits: u32) -> Self {
        Self {
            sat: self.sat || rhs.sat,
            value: self.value.add(&rhs.value, bits),
        }
    }
}
struct Table {
    scope: Vec<usize>,
    cells: Vec<Cell>,
}
struct Eq {
    output: usize,
    terms: Vec<Vec<usize>>,
}
struct Phase {
    variables: Vec<usize>,
    positive_weight: Complex,
}
struct Model {
    n: usize,
    outputs: Vec<Eq>,
    phases: Vec<Phase>,
}
#[derive(Default)]
struct Stats {
    peak_scope: usize,
    work_cells: usize,
    peak_factor_cells: usize,
    eliminations: usize,
}
const MAX_INPUT_BYTES: usize = 4_000_000;
const MAX_VARIABLES: usize = 256;
const MAX_SCOPE: usize = 16;
const MAX_CELLS: usize = 10_000_000;

fn number(s: &str) -> Result<usize, String> {
    s.parse::<usize>()
        .map_err(|_| "malformed factor number".into())
}
fn float(s: &str, bits: u32) -> Result<Float, String> {
    let parsed = Float::parse(s).map_err(|_| "malformed source MPFR weight")?;
    let value = Float::with_val(bits, parsed);
    if !value.is_finite() {
        return Err("nonfinite source MPFR weight".into());
    }
    Ok(value)
}
fn parse(text: &str, bits: u32) -> Result<Model, String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err("source factor file exceeds byte budget".into());
    }
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let header: Vec<_> = lines
        .first()
        .ok_or("missing source header")?
        .split_whitespace()
        .collect();
    if header.len() != 3 || header[0..2] != ["p", "pf"] {
        return Err("invalid source factor header".into());
    }
    let n = number(header[2])?;
    if n > MAX_VARIABLES {
        return Err("source factor dimensions exceeded".into());
    }
    let mut outputs = Vec::new();
    let mut phases = Vec::new();
    let mut i = 1;
    while i < lines.len() {
        let tokens: Vec<_> = lines[i].split_whitespace().collect();
        match tokens.first().copied() {
            Some("o") => {
                if tokens.len() != 3 {
                    return Err("bad output equation row".into());
                }
                let output = number(tokens[1])?;
                if output >= n {
                    return Err("output variable out of range".into());
                }
                let size = number(tokens[2])?;
                if size > 100_000 || size > lines.len().saturating_sub(i + 1) {
                    return Err("output monomial count exceeds source budget".into());
                }
                let mut terms = Vec::with_capacity(size);
                for _ in 0..size {
                    i += 1;
                    let monomial: Vec<_> = lines[i].split_whitespace().collect();
                    if monomial.len() < 2 || monomial[0] != "m" {
                        return Err("missing AND monomial".into());
                    }
                    let arity = number(monomial[1])?;
                    if arity > n || arity + 2 != monomial.len() {
                        return Err("invalid AND monomial size".into());
                    }
                    let vars = monomial[2..]
                        .iter()
                        .map(|s| number(s))
                        .collect::<Result<Vec<_>, _>>()?;
                    if vars.iter().any(|&v| v >= n) {
                        return Err("AND variable outside source range".into());
                    }
                    terms.push(vars);
                }
                outputs.push(Eq { output, terms });
            }
            Some("h") => {
                if tokens.len() < 4 {
                    return Err("incomplete phase factor".into());
                }
                let arity = number(tokens[3])?;
                if arity > n || arity + 4 != tokens.len() {
                    return Err("invalid phase monomial arity".into());
                }
                let variables = tokens[4..]
                    .iter()
                    .map(|s| number(s))
                    .collect::<Result<Vec<_>, _>>()?;
                if variables.iter().any(|&v| v >= n) {
                    return Err("phase variable out of range".into());
                }
                phases.push(Phase {
                    variables,
                    positive_weight: Complex {
                        re: float(tokens[1], bits)?,
                        im: float(tokens[2], bits)?,
                    },
                });
            }
            _ => return Err("unknown native factor record".into()),
        }
        i += 1;
    }
    Ok(Model { n, outputs, phases })
}
fn sorted(vars: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let mut v: Vec<_> = vars.into_iter().collect();
    v.sort_unstable();
    v.dedup();
    v
}
fn bit(scope: &[usize], assignment: usize, v: usize) -> bool {
    scope
        .binary_search(&v)
        .is_ok_and(|p| assignment & (1usize << p) != 0)
}
fn project(mask: usize, parent: &[usize], child: &[usize]) -> usize {
    child.iter().enumerate().fold(0, |acc, (i, &v)| {
        if bit(parent, mask, v) {
            acc | (1 << i)
        } else {
            acc
        }
    })
}
fn table(
    scope: Vec<usize>,
    stats: &mut Stats,
    mut make: impl FnMut(usize, &[usize]) -> Cell,
) -> Result<Table, String> {
    if scope.len() > MAX_SCOPE || scope.len() >= usize::BITS as usize {
        return Err("factor width exceeded".into());
    }
    let size = 1usize << scope.len();
    stats.work_cells = stats
        .work_cells
        .checked_add(size)
        .ok_or("work counter overflow")?;
    if stats.work_cells > MAX_CELLS {
        return Err("factor work budget exceeded".into());
    }
    stats.peak_scope = stats.peak_scope.max(scope.len());
    stats.peak_factor_cells = stats.peak_factor_cells.max(size);
    Ok(Table {
        cells: (0..size).map(|a| make(a, &scope)).collect(),
        scope,
    })
}
fn solve(p: &Model, bits: u32) -> Result<(Cell, Stats), String> {
    let mut stats = Stats::default();
    let mut factors = Vec::new();
    for equation in &p.outputs {
        let scope = sorted(
            std::iter::once(equation.output)
                .chain(equation.terms.iter().flat_map(|term| term.iter().copied())),
        );
        factors.push(table(scope, &mut stats, |a, scope| {
            let expected = equation.terms.iter().fold(false, |acc, term| {
                acc ^ term.iter().all(|&v| bit(scope, a, v))
            });
            if bit(scope, a, equation.output) == expected {
                Cell::one(bits)
            } else {
                Cell::zero(bits)
            }
        })?);
    }
    for phase in &p.phases {
        let scope = sorted(phase.variables.iter().copied());
        factors.push(table(scope, &mut stats, |a, scope| Cell {
            sat: true,
            value: if phase.variables.iter().all(|&v| bit(scope, a, v)) {
                phase.positive_weight.clone()
            } else {
                Complex::one(bits)
            },
        })?);
    }
    let mut pending: Vec<usize> = (0..p.n).collect();
    while !pending.is_empty() {
        let pivot = *pending
            .iter()
            .min_by_key(|&&var| {
                let scope = sorted(
                    factors
                        .iter()
                        .filter(|t| t.scope.binary_search(&var).is_ok())
                        .flat_map(|t| t.scope.iter().copied()),
                );
                (scope.len(), var)
            })
            .ok_or("missing elimination pivot")?;
        let mut rest = Vec::new();
        let mut touched = Vec::new();
        for f in factors {
            if f.scope.binary_search(&pivot).is_ok() {
                touched.push(f);
            } else {
                rest.push(f);
            }
        }
        let scope = if touched.is_empty() {
            vec![pivot]
        } else {
            sorted(touched.iter().flat_map(|t| t.scope.iter().copied()))
        };
        let joined = table(scope.clone(), &mut stats, |a, vars| {
            touched.iter().fold(Cell::one(bits), |acc, factor| {
                acc.product(&factor.cells[project(a, vars, &factor.scope)], bits)
            })
        })?;
        let pos = scope
            .binary_search(&pivot)
            .map_err(|_| "missing pivot scope")?;
        let mut projected_scope = scope;
        projected_scope.remove(pos);
        let projected = table(projected_scope, &mut stats, |a, _| {
            let before = (1usize << pos) - 1;
            let base = (a & before) | ((a & !before) << 1);
            joined.cells[base].sum(&joined.cells[base | (1 << pos)], bits)
        })?;
        rest.push(projected);
        factors = rest;
        pending.retain(|&v| v != pivot);
        stats.eliminations += 1;
    }
    let result = factors
        .iter()
        .fold(Cell::one(bits), |acc, t| acc.product(&t.cells[0], bits));
    Ok((result, stats))
}

/// Restricted source-ANF Walsh expansion for a phase-free, unweighted trace.
/// This is exact integer model counting, not a general MPFR phase shortcut.
/// No dense 2^width table is allocated; the work scales as 2^r monomials.
fn single_anf_walsh(p: &Model, bits: u32) -> Result<(Cell, Stats), String> {
    use std::collections::BTreeSet;
    if !p.phases.is_empty() || p.n > 50 {
        return Err("ANF_WALSH_INELIGIBLE_PHASE_OR_DIMENSION".into());
    }
    let mut nontrivial: Option<(bool, Vec<Vec<usize>>)> = None;
    for equation in &p.outputs {
        let mut polynomial = BTreeSet::<Vec<usize>>::new();
        let mut toggle = |term: &[usize]| {
            let key = sorted(term.iter().copied());
            if !polynomial.insert(key.clone()) {
                polynomial.remove(&key);
            }
        };
        toggle(&[equation.output]);
        for term in &equation.terms {
            toggle(term);
        }
        let sign_negative = polynomial.remove(&Vec::new());
        if polynomial.is_empty() && !sign_negative {
            continue;
        }
        if nontrivial.is_some() {
            return Err("ANF_WALSH_INELIGIBLE_MULTIPLE_EQUATIONS".into());
        }
        if polynomial.len() > 20 {
            return Err("ANF_WALSH_INELIGIBLE_TOO_MANY_MONOMIALS".into());
        }
        nontrivial = Some((sign_negative, polynomial.into_iter().collect()));
    }
    let total = 1i128 << p.n;
    let (models, work) = if let Some((negative, monomials)) = nontrivial {
        let masks: Vec<u64> = monomials
            .iter()
            .map(|term| term.iter().fold(0u64, |mask, &v| mask | (1u64 << v)))
            .collect();
        let subsets = 1usize << masks.len();
        let mut walsh = 0i128;
        for subset in 0..subsets {
            let mut support = 0u64;
            for (i, &mask) in masks.iter().enumerate() {
                if subset & (1usize << i) != 0 {
                    support |= mask;
                }
            }
            let free = p.n - support.count_ones() as usize;
            let contribution = (1i128 << subset.count_ones()) << free;
            if subset.count_ones() & 1 == 0 {
                walsh += contribution;
            } else {
                walsh -= contribution;
            }
        }
        let double = if negative {
            total - walsh
        } else {
            total + walsh
        };
        if double < 0 || double > 2 * total || double % 2 != 0 {
            return Err("ANF_WALSH_NONPHYSICAL_COUNT".into());
        }
        (double / 2, subsets)
    } else {
        (total, 0)
    };
    Ok((
        Cell {
            sat: models != 0,
            value: Complex {
                re: Float::with_val(bits, models as i64),
                im: Float::with_val(bits, 0),
            },
        },
        Stats {
            peak_scope: 0,
            work_cells: work,
            peak_factor_cells: 0,
            eliminations: 0,
        },
    ))
}

fn run() -> Result<(), String> {
    let mut argv = env::args_os().skip(1);
    let first = argv
        .next()
        .ok_or("usage: [--anf-walsh] PRECISION_DIGITS FACTOR_FILE")?;
    let (use_walsh, precision) = if first == "--anf-walsh" {
        (
            true,
            argv.next().ok_or("precision required after --anf-walsh")?,
        )
    } else {
        (false, first)
    };
    let file = argv.next().ok_or("missing factor file")?;
    if argv.next().is_some() {
        return Err("unexpected argument".into());
    }
    let digits: usize = precision
        .to_str()
        .ok_or("non-UTF8 precision")?
        .parse()
        .map_err(|_| "invalid precision digits")?;
    let bits: u32 = match digits {
        30 => 100,
        100 => 333,
        _ => return Err("precision must be 30 or 100 decimal digits".into()),
    };
    let started = Instant::now();
    let raw = fs::read_to_string(file).map_err(|e| e.to_string())?;
    let problem = parse(&raw, bits)?;
    let parse_ns = started.elapsed().as_nanos();
    let compute = Instant::now();
    let (result, stats) = if use_walsh {
        single_anf_walsh(&problem, bits)?
    } else {
        solve(&problem, bits)?
    };
    let solve_ns = compute.elapsed().as_nanos();
    println!(
        "s {}",
        if result.sat {
            "SATISFIABLE"
        } else {
            "UNSATISFIABLE"
        }
    );
    println!(
        "c r mpfr {} {}",
        result.value.re.to_string_radix(10, Some(digits + 8)),
        result.value.im.to_string_radix(10, Some(digits + 8))
    );
    eprintln!(
        "RESEARCH_FACTOR_MPFR_PHASE_NS parse={parse_ns} solve={solve_ns} \
         bits={bits} method={} factors={} peak_scope={} visited_cells={} eliminations={}",
        if use_walsh {
            "ANF_WALSH"
        } else {
            "NATIVE_FACTOR_DP"
        },
        problem.outputs.len() + problem.phases.len(),
        stats.peak_scope,
        stats.work_cells,
        stats.eliminations
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("RESEARCH_FACTOR_MPFR_UNQUALIFIED {e}");
        std::process::exit(2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn verified(source: &str, bits: u32, expected_sat: bool, re: i32, im: i32) {
        let p = parse(source, bits).unwrap();
        let (v, _) = solve(&p, bits).unwrap();
        assert_eq!(v.sat, expected_sat);
        assert_eq!(v.value.re, Float::with_val(bits, re));
        assert_eq!(v.value.im, Float::with_val(bits, im));
    }
    #[test]
    fn general_phase_weight_and_free_variable() {
        verified("p pf 2\nh 0 1 1 0\n", 100, true, 2, 2);
        verified("p pf 3\n", 333, true, 8, 0);
    }
    #[test]
    fn source_output_constraint_and_sign_cancellation() {
        verified("p pf 2\no 0 1\nm 1 1\nh -1 0 1 0\n", 100, true, 0, 0);
        verified("p pf 1\no 0 0\nh 0.1 0.2 1 0\n", 333, true, 1, 0);
        verified("p pf 1\no 0 0\nh 1 0 0\n", 100, true, 1, 0);
    }
    #[test]
    fn unsat_is_not_numerical_zero() {
        verified("p pf 1\no 0 0\no 0 1\nm 0\n", 100, false, 0, 0);
    }
    #[test]
    fn source_toffoli10_walsh_and_independent_full_enumeration() {
        // The pinned source has 19 variables and one equation containing
        // nine nontrivial AND monomials; the dense initial scope is 19.
        // Here also compare every assignment against a separate evaluator.
        let terms = [
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            vec![2, 3, 4, 5, 6, 7, 8, 9, 10],
            vec![3, 4, 5, 6, 7, 8, 9, 11],
            vec![4, 5, 6, 7, 8, 9, 12],
            vec![5, 6, 7, 8, 9, 13],
            vec![6, 7, 8, 9, 14],
            vec![7, 8, 9, 15],
            vec![8, 9, 16],
            vec![9, 17],
        ];
        let mut input = String::from("p pf 19\no 18 10\nm 1 18\n");
        for term in &terms {
            input.push_str(&format!("m {}", term.len()));
            for v in term {
                input.push_str(&format!(" {v}"));
            }
            input.push('\n');
        }
        let model = parse(&input, 100).unwrap();
        assert!(solve(&model, 100).is_err());
        let (count, work) = single_anf_walsh(&model, 100).unwrap();
        let mut oracle = 0i64;
        for bits in 0usize..(1 << 19) {
            let parity = terms.iter().fold(false, |v, term| {
                v ^ term.iter().all(|&x| bits & (1 << x) != 0)
            });
            if !parity {
                oracle += 1;
            }
        }
        assert_eq!(oracle, 393_216);
        assert!(count.sat);
        assert_eq!(count.value.re, Float::with_val(100, oracle));
        assert_eq!(count.value.im, Float::with_val(100, 0));
        assert_eq!(work.work_cells, 512);
    }

    #[test]
    fn walsh_rejects_phases_and_multiple_nontrivial_outputs() {
        let phase = parse("p pf 2\nh -1 0 1 0\n", 100).unwrap();
        assert!(single_anf_walsh(&phase, 100).is_err());
        let multi = parse("p pf 3\no 0 1\nm 1 1\no 1 1\nm 1 2\n", 100).unwrap();
        assert!(single_anf_walsh(&multi, 100).is_err());
        let tautology = parse("p pf 3\no 0 1\nm 1 0\n", 333).unwrap();
        let (result, _) = single_anf_walsh(&tautology, 333).unwrap();
        assert!(result.sat);
        assert_eq!(result.value.re, Float::with_val(333, 8));
    }

    #[test]
    fn width_cap_and_bad_source_fail_closed() {
        let vars = (0..18).map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
        let text = format!("p pf 18\nh 1 0 18 {vars}\n");
        assert!(solve(&parse(&text, 100).unwrap(), 100).is_err());
        for bad in [
            "p pf 1\nh NaN 0 0\n",
            "p pf 2\nh 1 0 2 0 5\n",
            "p pf 2\no 1 1\n",
        ] {
            assert!(parse(bad, 100).is_err());
        }
    }
}
