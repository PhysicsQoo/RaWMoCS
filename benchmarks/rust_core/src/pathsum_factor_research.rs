//! Research-only direct Path-sum factors; no CNF/Tseitin materialization.
//! All weights are exact printed Gaussian rationals, not source-exact angles.
//! Bounded project-join elimination is intentionally not a production backend.
use num_bigint::BigInt;
use num_rational::BigRational;
use rawmocs_core::{Gaussian, Weights};

#[derive(Clone, Debug)]
pub struct Parity {
    pub variables: Vec<usize>,
    pub rhs: bool,
}

#[derive(Clone, Debug)]
pub struct PhaseMonomial {
    pub variables: Vec<usize>,
    pub weight_if_true: Gaussian,
}

#[derive(Clone, Debug)]
pub struct OutputEquality {
    pub output: usize,
    pub terms: Vec<Vec<usize>>,
    pub constant: bool,
}

#[derive(Clone, Debug)]
pub struct PathsumFactors {
    pub variables: usize,
    pub weights: Vec<Weights>,
    pub parity: Vec<Parity>,
    pub phase: Vec<PhaseMonomial>,
    pub outputs: Vec<OutputEquality>,
    pub scalar: Gaussian,
}

#[derive(Clone, Copy, Debug)]
pub struct FactorLimits {
    pub max_scope: usize,
    pub max_work_cells: usize,
}

impl Default for FactorLimits {
    fn default() -> Self {
        Self {
            max_scope: 12,
            max_work_cells: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FactorStats {
    pub input_factors: usize,
    pub peak_scope: usize,
    pub peak_entries: usize,
    pub visited_cells: usize,
    pub eliminations: usize,
}

#[derive(Clone, Debug)]
pub struct FactorReport {
    pub satisfiable: bool,
    pub count: Gaussian,
    pub stats: FactorStats,
}

#[derive(Clone, Debug)]
pub struct StructuralProfile {
    pub variables: usize,
    pub parity_constraints: usize,
    pub output_constraints: usize,
    pub phase_terms: usize,
    pub max_phase_degree: usize,
    pub max_anf_degree: usize,
    pub max_constraint_scope: usize,
    pub primal_minfill_width_upper: usize,
}

#[derive(Clone, Debug)]
struct Cell {
    sat: bool,
    count: Gaussian,
}

impl Cell {
    fn one() -> Self {
        Self {
            sat: true,
            count: Gaussian::one(),
        }
    }

    fn zero() -> Self {
        Self {
            sat: false,
            count: Gaussian::zero(),
        }
    }

    fn product(&self, other: &Self) -> Self {
        Self {
            sat: self.sat && other.sat,
            count: self.count.mul(&other.count),
        }
    }

    fn sum(&self, other: &Self) -> Self {
        Self {
            sat: self.sat || other.sat,
            count: self.count.add(&other.count),
        }
    }
}

#[derive(Clone, Debug)]
struct Table {
    scope: Vec<usize>,
    cells: Vec<Cell>,
}

fn sorted_scope(vars: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let mut scope: Vec<usize> = vars.into_iter().collect();
    scope.sort_unstable();
    scope.dedup();
    scope
}

fn bit(scope: &[usize], assignment: usize, variable: usize) -> bool {
    scope
        .binary_search(&variable)
        .is_ok_and(|position| assignment & (1 << position) != 0)
}

fn project_mask(assignment: usize, from: &[usize], to: &[usize]) -> usize {
    to.iter().enumerate().fold(0, |result, (position, &var)| {
        if bit(from, assignment, var) {
            result | (1 << position)
        } else {
            result
        }
    })
}

fn charge(stats: &mut FactorStats, limits: FactorLimits, entries: usize) -> Result<(), String> {
    stats.visited_cells = stats
        .visited_cells
        .checked_add(entries)
        .ok_or("factor work counter overflow")?;
    if stats.visited_cells > limits.max_work_cells {
        return Err("bounded factor work exceeded".into());
    }
    stats.peak_entries = stats.peak_entries.max(entries);
    Ok(())
}

fn build_table(
    scope: Vec<usize>,
    limits: FactorLimits,
    stats: &mut FactorStats,
    mut cell: impl FnMut(usize, &[usize]) -> Cell,
) -> Result<Table, String> {
    if scope.len() > limits.max_scope || scope.len() >= usize::BITS as usize {
        return Err("factor scope exceeds bounded admission".into());
    }
    let entries = 1usize << scope.len();
    charge(stats, limits, entries)?;
    stats.peak_scope = stats.peak_scope.max(scope.len());
    Ok(Table {
        cells: (0..entries).map(|a| cell(a, &scope)).collect(),
        scope,
    })
}

fn union_scopes(tables: &[Table]) -> Vec<usize> {
    sorted_scope(tables.iter().flat_map(|t| t.scope.iter().copied()))
}

impl PathsumFactors {
    pub fn validate(&self) -> Result<(), String> {
        if self.variables > 48 || self.weights.len() != self.variables {
            return Err("invalid factor variable count or weights".into());
        }
        let check = |vars: &[usize]| -> Result<(), String> {
            if vars.iter().any(|&v| v >= self.variables) {
                Err("factor variable outside declared range".into())
            } else {
                Ok(())
            }
        };
        for p in &self.parity {
            check(&p.variables)?;
        }
        for p in &self.phase {
            check(&p.variables)?;
        }
        for e in &self.outputs {
            check(&[e.output])?;
            for monomial in &e.terms {
                check(monomial)?;
            }
        }
        Ok(())
    }

    fn constraint_scopes(&self) -> Vec<Vec<usize>> {
        let mut scopes = Vec::new();
        for p in &self.parity {
            scopes.push(sorted_scope(p.variables.iter().copied()));
        }
        for p in &self.phase {
            scopes.push(sorted_scope(p.variables.iter().copied()));
        }
        for e in &self.outputs {
            scopes.push(sorted_scope(
                std::iter::once(e.output).chain(e.terms.iter().flat_map(|m| m.iter().copied())),
            ));
        }
        scopes
    }

    pub fn profile(&self) -> Result<StructuralProfile, String> {
        self.validate()?;
        let scopes = self.constraint_scopes();
        let mut adjacency = vec![0u64; self.variables];
        for scope in &scopes {
            for &a in scope {
                for &b in scope {
                    if a != b {
                        adjacency[a] |= 1u64 << b;
                    }
                }
            }
        }
        let mut remaining: u64 = if self.variables == 0 {
            0
        } else {
            (1u64 << self.variables) - 1
        };
        let mut width = 0;
        while remaining != 0 {
            let candidate = (0..self.variables)
                .filter(|&v| remaining & (1 << v) != 0)
                .min_by_key(|&v| {
                    let neighbors = adjacency[v] & remaining & !(1 << v);
                    let mut fill = 0;
                    let mut a = neighbors;
                    while a != 0 {
                        let x = a.trailing_zeros() as usize;
                        a &= a - 1;
                        fill += (neighbors & !adjacency[x] & !(1 << x)).count_ones();
                    }
                    (fill, neighbors.count_ones(), v)
                })
                .ok_or("missing elimination candidate")?;
            let neighbors = adjacency[candidate] & remaining & !(1 << candidate);
            width = width.max(neighbors.count_ones() as usize);
            let mut bits = neighbors;
            while bits != 0 {
                let v = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                adjacency[v] |= neighbors & !(1 << v);
            }
            remaining &= !(1 << candidate);
        }
        Ok(StructuralProfile {
            variables: self.variables,
            parity_constraints: self.parity.len(),
            output_constraints: self.outputs.len(),
            phase_terms: self.phase.len(),
            max_phase_degree: self
                .phase
                .iter()
                .map(|p| p.variables.len())
                .max()
                .unwrap_or(0),
            max_anf_degree: self
                .outputs
                .iter()
                .flat_map(|o| o.terms.iter().map(Vec::len))
                .max()
                .unwrap_or(0),
            max_constraint_scope: scopes.iter().map(Vec::len).max().unwrap_or(0),
            primal_minfill_width_upper: width,
        })
    }

    pub fn solve_bounded(&self, limits: FactorLimits) -> Result<FactorReport, String> {
        self.validate()?;
        if limits.max_scope > 20 || limits.max_scope >= usize::BITS as usize {
            return Err("unadmitted factor width limit".into());
        }
        let mut stats = FactorStats::default();
        let mut tables = Vec::new();
        for (var, w) in self.weights.iter().enumerate() {
            tables.push(build_table(vec![var], limits, &mut stats, |a, _| Cell {
                sat: true,
                count: if a == 0 {
                    w.negative.clone()
                } else {
                    w.positive.clone()
                },
            })?);
        }
        for p in &self.parity {
            let scope = sorted_scope(p.variables.iter().copied());
            tables.push(build_table(scope, limits, &mut stats, |a, s| {
                let parity = p.variables.iter().filter(|&&v| bit(s, a, v)).count() % 2 != 0;
                if parity == p.rhs {
                    Cell::one()
                } else {
                    Cell::zero()
                }
            })?);
        }
        for p in &self.phase {
            let scope = sorted_scope(p.variables.iter().copied());
            tables.push(build_table(scope, limits, &mut stats, |a, s| Cell {
                sat: true,
                count: if p.variables.iter().all(|&v| bit(s, a, v)) {
                    p.weight_if_true.clone()
                } else {
                    Gaussian::one()
                },
            })?);
        }
        for e in &self.outputs {
            let scope = sorted_scope(
                std::iter::once(e.output).chain(e.terms.iter().flat_map(|t| t.iter().copied())),
            );
            tables.push(build_table(scope, limits, &mut stats, |a, s| {
                let mut expected = e.constant;
                for monomial in &e.terms {
                    expected ^= monomial.iter().all(|&v| bit(s, a, v));
                }
                if bit(s, a, e.output) == expected {
                    Cell::one()
                } else {
                    Cell::zero()
                }
            })?);
        }
        stats.input_factors = tables.len();
        let mut remaining: Vec<usize> = (0..self.variables).collect();
        while !remaining.is_empty() {
            let pivot = *remaining
                .iter()
                .min_by_key(|&&var| {
                    let scopes = tables
                        .iter()
                        .filter(|t| t.scope.binary_search(&var).is_ok());
                    let united = sorted_scope(scopes.flat_map(|t| t.scope.iter().copied()));
                    (united.len(), var)
                })
                .ok_or("missing pivot")?;
            let mut rest = Vec::new();
            let mut active = Vec::new();
            for table in tables {
                if table.scope.binary_search(&pivot).is_ok() {
                    active.push(table);
                } else {
                    rest.push(table);
                }
            }
            let scope = union_scopes(&active);
            let combined = build_table(scope.clone(), limits, &mut stats, |a, s| {
                active.iter().fold(Cell::one(), |acc, table| {
                    acc.product(&table.cells[project_mask(a, s, &table.scope)])
                })
            })?;
            let position = scope.binary_search(&pivot).map_err(|_| "pivot absent")?;
            let mut next_scope = scope;
            next_scope.remove(position);
            let projected = build_table(next_scope, limits, &mut stats, |a, _| {
                let low_mask = (1usize << position) - 1;
                let base = (a & low_mask) | ((a & !low_mask) << 1);
                combined.cells[base].sum(&combined.cells[base | (1 << position)])
            })?;
            rest.push(projected);
            remaining.retain(|&v| v != pivot);
            stats.eliminations += 1;
            tables = rest;
        }
        let final_value = tables
            .iter()
            .fold(Cell::one(), |acc, t| acc.product(&t.cells[0]));
        Ok(FactorReport {
            satisfiable: final_value.sat,
            count: final_value.count.mul(&self.scalar),
            stats,
        })
    }
}

/// Independent source-trace specialization for low-count monotone forbidden
/// conjunctions. This does not expand ANF to generic SAT CNF or run CDCL.
///
/// The full trace condition is x_i = f_i(x) for every output i. XOR-cancel
/// the left-hand x_i with the ANF right-hand side (idempotent monomials).
/// Admit only: tautology, contradiction, or one remaining AND monomial
/// that is required to be false. For forbidden monomials M_j, exact WMC
/// over *unweighted* variables is:
///
///  sum_{S subset forbidden} (-1)^|S| * 2^(n - |union(M_j : j in S)|).
///
/// Unlike dense factor tables this is exponential in the number of
/// effective forbidden conjunctions, NOT their degrees. All guards fail
/// closed; phase terms, parity, and nonunit literal weights are rejected.
#[derive(Clone, Debug)]
pub struct MonotoneTraceReport {
    pub satisfiable: bool,
    pub count: Gaussian,
    pub forbidden_conjunctions: usize,
    pub subsets_evaluated: usize,
}

impl PathsumFactors {
    pub fn count_monotone_trace_bounded(&self) -> Result<MonotoneTraceReport, String> {
        self.validate()?;
        if self.variables > 50 {
            return Err("monotone trace variable budget exceeded".into());
        }
        if !self.parity.is_empty() || !self.phase.is_empty() {
            return Err("monotone trace requires phase-free, parity-free inputs".into());
        }
        let one = Gaussian::one();
        if self
            .weights
            .iter()
            .any(|w| w.positive != one || w.negative != one)
        {
            return Err("monotone trace requires unit literal weights".into());
        }

        let mut forbidden = std::collections::BTreeSet::new();
        for equation in &self.outputs {
            // A Boolean monomial is AND-idempotent; parity of duplicate
            // ANF monomials cancels in GF(2), even if source is unnormalized.
            let mut parity: std::collections::BTreeSet<Vec<usize>> =
                std::collections::BTreeSet::new();
            let mut toggle = |mut term: Vec<usize>| {
                term.sort_unstable();
                term.dedup();
                if !parity.insert(term.clone()) {
                    parity.remove(&term);
                }
            };
            toggle(vec![equation.output]);
            for term in &equation.terms {
                toggle(term.clone());
            }
            if equation.constant {
                toggle(Vec::new());
            }
            if parity.is_empty() {
                continue;
            }
            if parity.len() != 1 {
                return Err("source ANF trace is not a monotone-conjunction subset".into());
            }
            let conjunction = parity
                .into_iter()
                .next()
                .ok_or("internal parity count changed")?;
            if conjunction.is_empty() {
                return Ok(MonotoneTraceReport {
                    satisfiable: false,
                    count: Gaussian::zero(),
                    forbidden_conjunctions: 1,
                    subsets_evaluated: 0,
                });
            }
            let mask = conjunction
                .iter()
                .fold(0u64, |bits, &var| bits | (1u64 << var));
            forbidden.insert(mask);
            if forbidden.len() > 20 {
                return Err("monotone trace inclusion-exclusion budget exceeded".into());
            }
        }

        let masks: Vec<u64> = forbidden.into_iter().collect();
        let subsets = 1usize << masks.len();
        let mut sum = 0i128;
        for selected in 0..subsets {
            let mut union = 0u64;
            for (j, &mask) in masks.iter().enumerate() {
                if selected & (1 << j) != 0 {
                    union |= mask;
                }
            }
            let free = self.variables - union.count_ones() as usize;
            let term = 1i128 << free;
            if selected.count_ones() % 2 == 0 {
                sum += term;
            } else {
                sum -= term;
            }
        }
        if !(0..=i64::MAX as i128).contains(&sum) {
            return Err("monotone trace count outside admitted integer range".into());
        }
        Ok(MonotoneTraceReport {
            satisfiable: sum > 0,
            count: Gaussian::integers(sum as i64, 0).mul(&self.scalar),
            forbidden_conjunctions: masks.len(),
            subsets_evaluated: subsets,
        })
    }
}

#[cfg(test)]
mod monotone_trace_tests {
    use super::*;

    fn sample(n: usize, output_equations: Vec<OutputEquality>) -> PathsumFactors {
        PathsumFactors {
            variables: n,
            weights: vec![Weights::new(Gaussian::one(), Gaussian::one()); n],
            parity: vec![],
            phase: vec![],
            outputs: output_equations,
            scalar: Gaussian::one(),
        }
    }

    fn brute_count(p: &PathsumFactors) -> i64 {
        let mut count = 0;
        for assignment in 0..(1usize << p.variables) {
            let bit = |v: usize| assignment & (1usize << v) != 0;
            if p.outputs.iter().all(|eq| {
                bit(eq.output)
                    == eq.terms.iter().fold(eq.constant, |acc, term| {
                        acc ^ term.iter().all(|&v| bit(v))
                    })
            }) {
                count += 1;
            }
        }
        count
    }

    #[test]
    fn monotone_trace_matches_independent_truth_tables_and_parity_cancellation() {
        for seed in 0usize..96 {
            let n = 7;
            let mut equations = Vec::new();
            for j in 0..3 {
                let output = (seed + j) % n;
                let monomial: Vec<_> = (0..n)
                    .filter(|&i| (seed.wrapping_mul(31) + j * 17 + i * 13) & (1 << i) != 0)
                    .collect();
                equations.push(OutputEquality {
                    output,
                    constant: false,
                    terms: vec![vec![output], monomial],
                });
            }
            let problem = sample(n, equations);
            let actual = problem.count_monotone_trace_bounded().unwrap();
            assert_eq!(
                actual.count,
                Gaussian::integers(brute_count(&problem), 0),
                "seed {seed}"
            );
            assert_eq!(actual.satisfiable, brute_count(&problem) > 0);
        }
    }

    #[test]
    fn high_degree_source_trace_does_not_need_dense_factor_table() {
        let p = sample(
            19,
            vec![OutputEquality {
                output: 18,
                constant: false,
                terms: vec![vec![18], (0..19).collect()],
            }],
        );
        assert!(p.solve_bounded(FactorLimits::default()).is_err());
        let exact = p.count_monotone_trace_bounded().unwrap();
        assert_eq!(exact.count, Gaussian::integers(524287, 0));
        assert!(exact.satisfiable);
        assert_eq!(exact.forbidden_conjunctions, 1);
        assert_eq!(exact.subsets_evaluated, 2);
    }

    #[test]
    fn source_phase_or_nonmonotone_anf_must_fail_closed() {
        let mut p = sample(
            4,
            vec![OutputEquality {
                output: 3,
                constant: false,
                terms: vec![vec![3], vec![0, 1], vec![1, 2]],
            }],
        );
        assert!(p.count_monotone_trace_bounded().is_err());
        p.outputs[0].terms.pop();
        p.phase.push(PhaseMonomial {
            variables: vec![0],
            weight_if_true: Gaussian::integers(-1, 0),
        });
        assert!(p.count_monotone_trace_bounded().is_err());
        p.phase.clear();
        p.weights[0] = Weights::new(Gaussian::integers(0, 0), Gaussian::one());
        assert!(p.count_monotone_trace_bounded().is_err());
    }

    #[test]
    fn constant_contradiction_and_zero_scalar_preserve_sat_flag() {
        let unsat = sample(
            3,
            vec![OutputEquality {
                output: 0,
                constant: true,
                terms: vec![vec![0]],
            }],
        );
        let a = unsat.count_monotone_trace_bounded().unwrap();
        assert!(!a.satisfiable);
        assert_eq!(a.count, Gaussian::zero());

        let mut sat = sample(
            3,
            vec![OutputEquality {
                output: 0,
                constant: false,
                terms: vec![vec![0], vec![1, 2]],
            }],
        );
        sat.scalar = Gaussian::zero();
        let b = sat.count_monotone_trace_bounded().unwrap();
        assert!(b.satisfiable);
        assert_eq!(b.count, Gaussian::zero());
    }
}

/// Restricted sign-only quadratic phase: (-1)^(c + l.x + sum B_ij x_i x_j).
/// Unlike a general Clifford+T phase, this supports only phases +1/-1.
#[derive(Clone, Debug)]
pub struct QuadraticSign {
    pub variables: usize,
    pub equations: Vec<Parity>,
    pub constant: bool,
    pub linear: Vec<usize>,
    pub quadratic: Vec<(usize, usize)>,
    pub scalar: Gaussian,
}

#[derive(Clone, Debug)]
pub struct QuadraticReport {
    pub satisfiable: bool,
    pub count: Gaussian,
    pub free_variables: usize,
    pub eliminated_pairs: usize,
}

impl QuadraticSign {
    fn eval(&self, assignment: u64) -> bool {
        let mut result = self.constant;
        for &v in &self.linear {
            result ^= assignment & (1u64 << v) != 0;
        }
        for &(a, b) in &self.quadratic {
            result ^= assignment & (1u64 << a) != 0 && assignment & (1u64 << b) != 0;
        }
        result
    }

    pub fn solve(&self) -> Result<QuadraticReport, String> {
        if self.variables > 60
            || self.linear.iter().any(|&v| v >= self.variables)
            || self
                .quadratic
                .iter()
                .any(|&(a, b)| a >= self.variables || b >= self.variables)
            || self
                .equations
                .iter()
                .flat_map(|e| e.variables.iter())
                .any(|&v| v >= self.variables)
        {
            return Err("invalid quadratic dimensions or variables".into());
        }
        let mut rows: Vec<(u64, bool)> = self
            .equations
            .iter()
            .map(|e| {
                let bits = e.variables.iter().fold(0u64, |m, &v| m ^ (1u64 << v));
                (bits, e.rhs)
            })
            .collect();
        let mut pivots = Vec::new();
        let mut at = 0;
        for col in 0..self.variables {
            if let Some(row) = (at..rows.len()).find(|&r| rows[r].0 & (1 << col) != 0) {
                rows.swap(at, row);
                let pivot = rows[at];
                for (r, row) in rows.iter_mut().enumerate() {
                    if r != at && row.0 & (1 << col) != 0 {
                        row.0 ^= pivot.0;
                        row.1 ^= pivot.1;
                    }
                }
                pivots.push(col);
                at += 1;
            }
        }
        if rows.iter().any(|&(lhs, rhs)| lhs == 0 && rhs) {
            return Ok(QuadraticReport {
                satisfiable: false,
                count: Gaussian::zero(),
                free_variables: 0,
                eliminated_pairs: 0,
            });
        }
        let mut particular = 0u64;
        for (row, &p) in pivots.iter().enumerate() {
            if rows[row].1 {
                particular |= 1 << p;
            }
        }
        let free: Vec<usize> = (0..self.variables)
            .filter(|v| !pivots.contains(v))
            .collect();
        let basis: Vec<u64> = free
            .iter()
            .map(|&f| {
                pivots.iter().enumerate().fold(1u64 << f, |mask, (r, &p)| {
                    if rows[r].0 & (1u64 << f) != 0 {
                        mask | (1u64 << p)
                    } else {
                        mask
                    }
                })
            })
            .collect();
        let k = free.len();
        let mut constant = self.eval(particular);
        let mut linear = vec![false; k];
        let mut edges = vec![0u64; k];
        for i in 0..k {
            linear[i] = self.eval(particular ^ basis[i]) ^ constant;
        }
        for i in 0..k {
            for j in (i + 1)..k {
                let cross = self.eval(particular ^ basis[i] ^ basis[j])
                    ^ self.eval(particular ^ basis[i])
                    ^ self.eval(particular ^ basis[j])
                    ^ constant;
                if cross {
                    edges[i] |= 1 << j;
                    edges[j] |= 1 << i;
                }
            }
        }
        let mut active = (1u64 << k) - 1;
        let mut exponent = 0usize;
        let mut eliminated_pairs = 0;
        while active != 0 {
            let i = active.trailing_zeros() as usize;
            let neighbors = edges[i] & active & !(1u64 << i);
            if neighbors == 0 {
                if linear[i] {
                    return Ok(QuadraticReport {
                        satisfiable: true,
                        count: Gaussian::zero(),
                        free_variables: k,
                        eliminated_pairs,
                    });
                }
                exponent += 1;
                active &= !(1 << i);
                continue;
            }
            let j = neighbors.trailing_zeros() as usize;
            let rest = active & !(1 << i) & !(1 << j);
            let ai = linear[i];
            let aj = linear[j];
            constant ^= ai && aj;
            let mut ks = rest;
            while ks != 0 {
                let v = ks.trailing_zeros() as usize;
                ks &= ks - 1;
                let iv = edges[i] & (1 << v) != 0;
                let jv = edges[j] & (1 << v) != 0;
                linear[v] ^= (ai && jv) ^ (aj && iv) ^ (iv && jv);
            }
            let mut ks = rest;
            while ks != 0 {
                let v = ks.trailing_zeros() as usize;
                ks &= ks - 1;
                let mut ls = rest & !((1u64 << (v + 1)) - 1);
                while ls != 0 {
                    let w = ls.trailing_zeros() as usize;
                    ls &= ls - 1;
                    let flip = ((edges[i] & (1 << v) != 0) && (edges[j] & (1 << w) != 0))
                        ^ ((edges[i] & (1 << w) != 0) && (edges[j] & (1 << v) != 0));
                    if flip {
                        edges[v] ^= 1 << w;
                        edges[w] ^= 1 << v;
                    }
                }
            }
            active = rest;
            exponent += 1;
            eliminated_pairs += 1;
        }
        let integer = BigInt::from(if constant { -1 } else { 1 }) << exponent;
        let value = Gaussian::new(
            BigRational::from_integer(integer),
            BigRational::from_integer(BigInt::from(0)),
        );
        Ok(QuadraticReport {
            satisfiable: true,
            count: value.mul(&self.scalar),
            free_variables: k,
            eliminated_pairs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_weights(n: usize) -> Vec<Weights> {
        vec![Weights::new(Gaussian::one(), Gaussian::one()); n]
    }

    fn oracle(p: &PathsumFactors) -> (bool, Gaussian) {
        assert!(p.variables <= 10);
        let mut total = Gaussian::zero();
        let mut sat = false;
        for mask in 0..(1usize << p.variables) {
            let bit = |v: usize| mask & (1 << v) != 0;
            if p.parity
                .iter()
                .any(|e| e.variables.iter().filter(|&&v| bit(v)).count() % 2 != usize::from(e.rhs))
            {
                continue;
            }
            if p.outputs.iter().any(|e| {
                let value = e
                    .terms
                    .iter()
                    .fold(e.constant, |b, term| b ^ term.iter().all(|&v| bit(v)));
                value != bit(e.output)
            }) {
                continue;
            }
            sat = true;
            let mut value = p.scalar.clone();
            for v in 0..p.variables {
                value = value.mul(if bit(v) {
                    &p.weights[v].positive
                } else {
                    &p.weights[v].negative
                });
            }
            for term in &p.phase {
                if term.variables.iter().all(|&v| bit(v)) {
                    value = value.mul(&term.weight_if_true);
                }
            }
            total = total.add(&value);
        }
        (sat, total)
    }

    fn sample() -> PathsumFactors {
        PathsumFactors {
            variables: 4,
            weights: vec![
                Weights::new(Gaussian::integers(1, 2), Gaussian::integers(-1, 0)),
                Weights::new(Gaussian::integers(0, 1), Gaussian::integers(2, 0)),
                Weights::new(Gaussian::integers(0, 0), Gaussian::integers(-1, 1)),
                Weights::new(Gaussian::integers(3, 0), Gaussian::integers(1, 0)),
            ],
            parity: vec![Parity {
                variables: vec![0, 1, 3],
                rhs: true,
            }],
            phase: vec![
                PhaseMonomial {
                    variables: vec![1, 2],
                    weight_if_true: Gaussian::integers(0, 1),
                },
                PhaseMonomial {
                    variables: vec![0, 2, 3],
                    weight_if_true: Gaussian::integers(-1, 0),
                },
            ],
            outputs: vec![OutputEquality {
                output: 3,
                terms: vec![vec![0, 1], vec![2], vec![]],
                constant: false,
            }],
            scalar: Gaussian::integers(0, -1),
        }
    }

    #[test]
    fn bounded_factor_matches_independent_full_complex_oracle() {
        let p = sample();
        let expected = oracle(&p);
        let actual = p.solve_bounded(FactorLimits::default()).unwrap();
        assert_eq!((actual.satisfiable, actual.count), expected);
        assert_eq!(actual.stats.eliminations, 4);
        let profile = p.profile().unwrap();
        assert_eq!(profile.max_phase_degree, 3);
        assert_eq!(profile.max_anf_degree, 2);
        assert!(profile.primal_minfill_width_upper >= 2);
    }

    #[test]
    fn free_variables_and_sat_but_zero_do_not_confuse_sat() {
        let mut p = PathsumFactors {
            variables: 3,
            weights: unit_weights(3),
            parity: vec![],
            phase: vec![],
            outputs: vec![],
            scalar: Gaussian::one(),
        };
        assert_eq!(
            p.solve_bounded(FactorLimits::default()).unwrap().count,
            Gaussian::integers(8, 0)
        );
        p.weights[0] = Weights::new(Gaussian::integers(1, 0), Gaussian::integers(-1, 0));
        let a = p.solve_bounded(FactorLimits::default()).unwrap();
        assert!(a.satisfiable);
        assert_eq!(a.count, Gaussian::zero());
        p.parity = vec![
            Parity {
                variables: vec![1],
                rhs: false,
            },
            Parity {
                variables: vec![1],
                rhs: true,
            },
        ];
        let b = p.solve_bounded(FactorLimits::default()).unwrap();
        assert!(!b.satisfiable);
        assert_eq!(b.count, Gaussian::zero());
    }

    #[test]
    fn width_budget_fails_closed_and_input_is_unchanged() {
        let p = PathsumFactors {
            variables: 14,
            weights: unit_weights(14),
            parity: vec![Parity {
                variables: (0..14).collect(),
                rhs: false,
            }],
            phase: vec![],
            outputs: vec![],
            scalar: Gaussian::one(),
        };
        let limits = FactorLimits {
            max_scope: 10,
            max_work_cells: 100000,
        };
        assert!(p.solve_bounded(limits).is_err());
        assert_eq!(p.parity[0].variables.len(), 14);
        assert!(p.profile().unwrap().primal_minfill_width_upper >= 13);
    }

    fn quadratic_oracle(p: &QuadraticSign) -> (bool, Gaussian) {
        let mut sat = false;
        let mut sum = 0i64;
        for mask in 0..(1u64 << p.variables) {
            let legal = p.equations.iter().all(|e| {
                e.variables
                    .iter()
                    .filter(|&&v| mask & (1 << v) != 0)
                    .count()
                    % 2
                    == usize::from(e.rhs)
            });
            if legal {
                sat = true;
                sum += if p.eval(mask) { -1 } else { 1 };
            }
        }
        (sat, Gaussian::integers(sum, 0).mul(&p.scalar))
    }

    #[test]
    fn quadratic_sign_elimination_matches_bruteforce_with_affine_constraints() {
        for seed in 0u64..100 {
            let mut p = QuadraticSign {
                variables: 6,
                equations: vec![],
                constant: seed & 1 != 0,
                linear: vec![],
                quadratic: vec![],
                scalar: Gaussian::integers(0, 1),
            };
            for v in 0..6 {
                if ((seed.wrapping_mul(13) + 7) >> v) & 1 != 0 {
                    p.linear.push(v);
                }
                for u in (v + 1)..6 {
                    if ((seed.wrapping_mul(67) + 11) >> (v + u)) & 1 != 0 {
                        p.quadratic.push((v, u));
                    }
                }
            }
            if seed & 2 != 0 {
                p.equations.push(Parity {
                    variables: vec![0, 1, 2],
                    rhs: seed & 4 != 0,
                });
            }
            if seed & 8 != 0 {
                p.equations.push(Parity {
                    variables: vec![2, 3, 5],
                    rhs: seed & 16 != 0,
                });
            }
            if seed & 32 != 0 {
                p.equations.push(Parity {
                    variables: vec![0, 1, 2],
                    rhs: seed & 4 == 0,
                });
            }
            let exact = quadratic_oracle(&p);
            let result = p.solve().unwrap();
            assert_eq!((result.satisfiable, result.count), exact, "seed {seed}");
        }
    }

    #[test]
    fn restricted_quadratic_phase_matches_factor_dp() {
        let q = QuadraticSign {
            variables: 4,
            equations: vec![Parity {
                variables: vec![0, 2, 3],
                rhs: true,
            }],
            constant: true,
            linear: vec![0, 1],
            quadratic: vec![(0, 2), (1, 3)],
            scalar: Gaussian::one(),
        };
        let f = PathsumFactors {
            variables: 4,
            weights: unit_weights(4),
            parity: q.equations.clone(),
            phase: q
                .linear
                .iter()
                .map(|&v| PhaseMonomial {
                    variables: vec![v],
                    weight_if_true: Gaussian::integers(-1, 0),
                })
                .chain(q.quadratic.iter().map(|&(a, b)| PhaseMonomial {
                    variables: vec![a, b],
                    weight_if_true: Gaussian::integers(-1, 0),
                }))
                .collect(),
            outputs: vec![],
            scalar: Gaussian::integers(-1, 0),
        };
        let a = q.solve().unwrap();
        let b = f.solve_bounded(FactorLimits::default()).unwrap();
        assert_eq!((a.satisfiable, a.count), (b.satisfiable, b.count));
    }
}
