//! Experimental exact, full complex weighted model counting over Q[i].
//! No projected-WMC or quantum-equivalence claims are made by this crate.

use num_rational::BigRational;
use num_traits::{One, Zero};
use std::collections::{BTreeMap, HashMap};
#[cfg(feature = "cache-cost-profile")]
use std::time::Instant;

type Clause = Vec<i32>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gaussian {
    pub real: BigRational,
    pub imag: BigRational,
}

impl Gaussian {
    pub fn new(real: BigRational, imag: BigRational) -> Self {
        Self { real, imag }
    }
    pub fn integers(real: i64, imag: i64) -> Self {
        Self::new(
            BigRational::from_integer(real.into()),
            BigRational::from_integer(imag.into()),
        )
    }
    pub fn zero() -> Self {
        Self::new(BigRational::zero(), BigRational::zero())
    }
    pub fn one() -> Self {
        Self::new(BigRational::one(), BigRational::zero())
    }
    pub fn add(&self, rhs: &Self) -> Self {
        Self::new(&self.real + &rhs.real, &self.imag + &rhs.imag)
    }
    pub fn mul(&self, rhs: &Self) -> Self {
        // Common multiplicative identities need no BigRational products or GCDs.
        // Keep the established four-product formula for general complex inputs.
        if self.imag.is_zero() {
            if self.real.is_zero() {
                return Self::zero();
            }
            if self.real.is_one() {
                return rhs.clone();
            }
        }
        if rhs.imag.is_zero() {
            if rhs.real.is_zero() {
                return Self::zero();
            }
            if rhs.real.is_one() {
                return self.clone();
            }
        }
        // Sparse factors avoid multiplying zero rational components.
        // The four-product expression remains unchanged for general inputs.
        if self.imag.is_zero() {
            return Self::new(&self.real * &rhs.real, &self.real * &rhs.imag);
        }
        if rhs.imag.is_zero() {
            return Self::new(&self.real * &rhs.real, &self.imag * &rhs.real);
        }
        if self.real.is_zero() {
            return Self::new(-(&self.imag * &rhs.imag), &self.imag * &rhs.real);
        }
        if rhs.real.is_zero() {
            return Self::new(-(&self.imag * &rhs.imag), &self.real * &rhs.imag);
        }
        Self::new(
            &self.real * &rhs.real - &self.imag * &rhs.imag,
            &self.real * &rhs.imag + &self.imag * &rhs.real,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Weights {
    pub positive: Gaussian,
    pub negative: Gaussian,
}
impl Weights {
    pub fn new(positive: Gaussian, negative: Gaussian) -> Self {
        Self { positive, negative }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProblemError {
    InvalidVariableCount,
    IncorrectWeightCount,
    InvalidLiteral(i32),
}
impl std::fmt::Display for ProblemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ProblemError {}

#[derive(Clone, Debug)]
pub struct Problem {
    variables: usize,
    clauses: Vec<Clause>,
    weights: Vec<Weights>,
}
impl Problem {
    pub fn new(
        variables: usize,
        clauses: Vec<Clause>,
        weights: Vec<Weights>,
    ) -> Result<Self, ProblemError> {
        if variables > i32::MAX as usize {
            return Err(ProblemError::InvalidVariableCount);
        }
        if weights.len() != variables {
            return Err(ProblemError::IncorrectWeightCount);
        }
        for clause in &clauses {
            for &lit in clause {
                if lit == 0 || lit.unsigned_abs() as usize > variables {
                    return Err(ProblemError::InvalidLiteral(lit));
                }
            }
        }
        Ok(Self {
            variables,
            clauses,
            weights,
        })
    }
    pub fn variables(&self) -> usize {
        self.variables
    }
    pub fn clauses(&self) -> &[Vec<i32>] {
        &self.clauses
    }
    pub fn weights(&self) -> &[Weights] {
        &self.weights
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveError {
    DepthBudgetExceeded,

    NodeBudgetExceeded,
}
impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SolveError {}
/// Instrumented *self-time* for cache transactions, enabled only by the
/// research-only cache-cost-profile feature. Timer and accounting overhead
/// perturb solve time; these counters must not be used as production timings.
#[cfg(feature = "cache-cost-profile")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheCostProfile {
    /// Copies the already-normalized residual and scope to construct a key.
    /// Excludes CNF normalization itself.
    pub key_build_calls: u64,
    pub key_build_ns: u128,
    /// Hashing and equality comparison inside HashMap::get only.
    pub lookup_calls: u64,
    pub lookup_ns: u128,
    /// Cloning a cached Outcome after a successful lookup.
    pub hit_clone_calls: u64,
    pub hit_clone_ns: u128,
    /// Logical payload bookkeeping, quota evaluation, and rejection.
    pub admission_calls: u64,
    pub admission_ns: u128,
    /// Outcome clone plus HashMap insertion and potential reallocation.
    pub insert_calls: u64,
    pub insert_ns: u128,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub cache_entries: usize,
    pub nodes: u64,
    pub cache_hits: u64,
    pub decompositions: u64,
    /// Sum of variable IDs stored in retained cache keys (not bytes).
    pub cache_scope_items: usize,
    /// Sum of clause vectors stored in retained cache keys (not bytes).
    pub cache_clause_items: usize,
    /// Sum of literals stored in retained cache key clauses (not bytes).
    pub cache_literal_items: usize,
    /// Sum of exact numeric bit lengths of the four BigInt components of
    /// cached Gaussian counts (numerator/denominator for real/imaginary).
    /// Excludes allocations, capacities, HashMap overhead, and temporaries.
    pub cache_value_bits: u64,
    /// Largest one-entry exact numeric bit-length within retained counts.
    pub max_cached_value_bits: u64,
    /// Number of cache entries declined by the optional retained logical
    /// payload budget. Does not include a full entry-count cache.
    pub cache_payload_rejections: u64,
    /// Research-only rejections based on predeclared normalized key clauses.
    #[cfg(feature = "cache-admission-research")]
    pub cache_min_clause_rejections: u64,
    #[cfg(feature = "cache-cost-profile")]
    pub cache_cost: CacheCostProfile,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub satisfiable: bool,
    pub count: Gaussian,
    pub stats: Stats,
}
#[derive(Clone)]
struct Outcome {
    sat: bool,
    count: Gaussian,
}
impl Outcome {
    fn unsat() -> Self {
        Self {
            sat: false,
            count: Gaussian::zero(),
        }
    }
}
#[derive(Hash, PartialEq, Eq)]
struct Key {
    scope: Vec<usize>,
    clauses: Vec<Clause>,
}
struct Engine<'a> {
    p: &'a Problem,
    budget: u64,
    depth_budget: usize,
    cache_budget: usize,
    cache_payload_budget: Option<CachePayloadBudget>,
    #[cfg(feature = "cache-admission-research")]
    min_cache_clauses: usize,
    stats: Stats,
    cache: HashMap<Key, Outcome>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub nodes: u64,
    pub depth: usize,
    pub cache_entries: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            nodes: 100_000,
            depth: 512,
            cache_entries: 10_000,
        }
    }
}
/// Cumulative admission quota for logical retained cache payload, NOT bytes.
///
/// This excludes HashMap buckets, Vec capacities, transient BigRational
/// values, input CNF, recursion and all process RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachePayloadBudget {
    pub scope_items: usize,
    pub clause_items: usize,
    pub literal_items: usize,
    pub value_bits: u64,
}

/// Original solver behavior: only cache-entry-count admissions are bounded.
pub fn solve_with_limits(p: &Problem, limits: Limits) -> Result<Report, SolveError> {
    solve_with_optional_cache_payload_budget(p, limits, None)
}

/// Exact Full WMC with an additional optional logical-cache payload quota.
///
/// Cache admissions may be declined without changing count, SAT, or errors.
/// This is not a hard memory, allocation, RSS, or OS resource limit.
pub fn solve_with_cache_payload_budget(
    p: &Problem,
    limits: Limits,
    budget: CachePayloadBudget,
) -> Result<Report, SolveError> {
    solve_with_optional_cache_payload_budget(p, limits, Some(budget))
}

/// Research-only structural admission: retain a cache entry only when its
/// normalized residual key has at least `min_clauses` distinct clauses.
/// Lookup still operates on every non-disabled cache key. Does not change
/// Full WMC, SAT, cancellation, node/depth budgets or default policies.
/// This fixed threshold is NOT a measured cost estimate.
#[cfg(feature = "cache-admission-research")]
pub fn solve_with_min_cache_clauses(
    p: &Problem,
    limits: Limits,
    min_clauses: usize,
) -> Result<Report, SolveError> {
    solve_with_research_cache_threshold(p, limits, None, min_clauses)
}

fn solve_with_optional_cache_payload_budget(
    p: &Problem,
    limits: Limits,
    cache_payload_budget: Option<CachePayloadBudget>,
) -> Result<Report, SolveError> {
    solve_with_research_cache_threshold(p, limits, cache_payload_budget, 0)
}

fn solve_with_research_cache_threshold(
    p: &Problem,
    limits: Limits,
    cache_payload_budget: Option<CachePayloadBudget>,
    min_cache_clauses: usize,
) -> Result<Report, SolveError> {
    #[cfg(not(feature = "cache-admission-research"))]
    let _ = min_cache_clauses;
    let mut engine = Engine {
        p,
        budget: limits.nodes,
        depth_budget: limits.depth,
        cache_budget: limits.cache_entries,
        cache_payload_budget,
        #[cfg(feature = "cache-admission-research")]
        min_cache_clauses,
        stats: Stats::default(),
        cache: HashMap::new(),
    };
    let answer = engine.count(p.clauses.clone(), (1..=p.variables).collect(), 0)?;
    Ok(Report {
        satisfiable: answer.sat,
        count: answer.count,
        stats: engine.stats,
    })
}
pub fn solve_with_budget(p: &Problem, budget: u64) -> Result<Report, SolveError> {
    solve_with_limits(
        p,
        Limits {
            nodes: budget,
            ..Limits::default()
        },
    )
}
pub fn solve(p: &Problem) -> Result<Report, SolveError> {
    solve_with_limits(p, Limits::default())
}

fn rational_value_bits(r: &BigRational) -> u64 {
    r.numer().bits().saturating_add(r.denom().bits())
}

fn gaussian_value_bits(value: &Gaussian) -> u64 {
    rational_value_bits(&value.real).saturating_add(rational_value_bits(&value.imag))
}

impl Engine<'_> {
    fn save(&mut self, key: Option<Key>, outcome: Outcome) -> Outcome {
        if let Some(key) = key {
            if self.cache.len() < self.cache_budget {
                #[cfg(feature = "cache-admission-research")]
                if key.clauses.len() < self.min_cache_clauses {
                    self.stats.cache_min_clause_rejections =
                        self.stats.cache_min_clause_rejections.saturating_add(1);
                    return outcome;
                }
                #[cfg(feature = "cache-cost-profile")]
                let admission_start = Instant::now();
                // These count stored logical payloads, NOT total allocated
                // memory or a bound on the process RSS.
                let scope = key.scope.len();
                let clauses = key.clauses.len();
                let literals = key
                    .clauses
                    .iter()
                    .fold(0usize, |sum, clause| sum.saturating_add(clause.len()));
                let bits = gaussian_value_bits(&outcome.count);
                if let Some(budget) = self.cache_payload_budget {
                    let allowed = self
                        .stats
                        .cache_scope_items
                        .checked_add(scope)
                        .is_some_and(|n| n <= budget.scope_items)
                        && self
                            .stats
                            .cache_clause_items
                            .checked_add(clauses)
                            .is_some_and(|n| n <= budget.clause_items)
                        && self
                            .stats
                            .cache_literal_items
                            .checked_add(literals)
                            .is_some_and(|n| n <= budget.literal_items)
                        && self
                            .stats
                            .cache_value_bits
                            .checked_add(bits)
                            .is_some_and(|n| n <= budget.value_bits);
                    if !allowed {
                        #[cfg(feature = "cache-cost-profile")]
                        {
                            self.stats.cache_cost.admission_calls += 1;
                            self.stats.cache_cost.admission_ns +=
                                admission_start.elapsed().as_nanos();
                        }
                        self.stats.cache_payload_rejections =
                            self.stats.cache_payload_rejections.saturating_add(1);
                        return outcome;
                    }
                }
                #[cfg(feature = "cache-cost-profile")]
                {
                    self.stats.cache_cost.admission_calls += 1;
                    self.stats.cache_cost.admission_ns += admission_start.elapsed().as_nanos();
                }
                #[cfg(feature = "cache-cost-profile")]
                let insert_start = Instant::now();
                self.cache.insert(key, outcome.clone());
                #[cfg(feature = "cache-cost-profile")]
                {
                    self.stats.cache_cost.insert_calls += 1;
                    self.stats.cache_cost.insert_ns += insert_start.elapsed().as_nanos();
                }
                self.stats.cache_entries = self.cache.len();
                self.stats.cache_scope_items = self.stats.cache_scope_items.saturating_add(scope);
                self.stats.cache_clause_items =
                    self.stats.cache_clause_items.saturating_add(clauses);
                self.stats.cache_literal_items =
                    self.stats.cache_literal_items.saturating_add(literals);
                self.stats.cache_value_bits = self.stats.cache_value_bits.saturating_add(bits);
                self.stats.max_cached_value_bits = self.stats.max_cached_value_bits.max(bits);
            }
        }
        outcome
    }
    fn weight(&self, var: usize, positive: bool) -> &Gaussian {
        let w = &self.p.weights[var - 1];
        if positive {
            &w.positive
        } else {
            &w.negative
        }
    }
    fn count(
        &mut self,
        clauses: Vec<Clause>,
        scope: Vec<usize>,
        depth: usize,
    ) -> Result<Outcome, SolveError> {
        if depth >= self.depth_budget {
            return Err(SolveError::DepthBudgetExceeded);
        }
        if self.stats.nodes >= self.budget {
            return Err(SolveError::NodeBudgetExceeded);
        }
        self.stats.nodes += 1;
        // With caching disabled, do not clone a scope or duplicate residual
        // clauses into a Key that will never be looked up or stored.
        let mut residual = normalize(clauses);
        let key = if self.cache_budget == 0 {
            None
        } else {
            #[cfg(feature = "cache-cost-profile")]
            let key_start = Instant::now();
            let key = Key {
                scope: scope.clone(),
                clauses: residual.clone(),
            };
            #[cfg(feature = "cache-cost-profile")]
            {
                self.stats.cache_cost.key_build_calls += 1;
                self.stats.cache_cost.key_build_ns += key_start.elapsed().as_nanos();
            }
            #[cfg(feature = "cache-cost-profile")]
            let lookup_start = Instant::now();
            let cached = self.cache.get(&key);
            #[cfg(feature = "cache-cost-profile")]
            {
                self.stats.cache_cost.lookup_calls += 1;
                self.stats.cache_cost.lookup_ns += lookup_start.elapsed().as_nanos();
            }
            if let Some(cached) = cached {
                self.stats.cache_hits += 1;
                #[cfg(feature = "cache-cost-profile")]
                let clone_start = Instant::now();
                let result = cached.clone();
                #[cfg(feature = "cache-cost-profile")]
                {
                    self.stats.cache_cost.hit_clone_calls += 1;
                    self.stats.cache_cost.hit_clone_ns += clone_start.elapsed().as_nanos();
                }
                return Ok(result);
            }
            Some(key)
        };

        let mut remaining = scope;
        let mut prefix = Gaussian::one();

        loop {
            if residual.iter().any(Vec::is_empty) {
                return Ok(self.save(key, Outcome::unsat()));
            }
            let Some(lit) = residual.iter().find(|c| c.len() == 1).map(|c| c[0]) else {
                break;
            };
            let var = lit.unsigned_abs() as usize;
            prefix = prefix.mul(self.weight(var, lit > 0));
            remaining.retain(|&v| v != var);
            residual = assign(&residual, lit);
        }

        // When unit propagation resolves every clause, all remaining
        // variables are free. Avoid allocating any residual frequency table.
        if residual.is_empty() {
            for var in remaining {
                let w = &self.p.weights[var - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative));
            }
            return Ok(self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix,
                },
            ));
        }

        // One scan serves two purposes: identify free variables and retain
        // the same literal-occurrence heuristic for the eventual DPLL pivot.
        let mut frequency = vec![0usize; self.p.variables + 1];
        for c in &residual {
            for &lit in c {
                frequency[lit.unsigned_abs() as usize] += 1;
            }
        }
        let mut active = Vec::new();
        for var in remaining {
            if frequency[var] != 0 {
                active.push(var);
            } else {
                let w = &self.p.weights[var - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative));
            }
        }

        if let Some(pieces) = split_components(&residual, &active, self.p.variables) {
            self.stats.decompositions += 1;
            let mut value = Gaussian::one();
            for (c, s) in pieces {
                let part = self.count(c, s, depth + 1)?;
                if !part.sat {
                    return Ok(self.save(key, Outcome::unsat()));
                }
                value = value.mul(&part.count);
            }
            return Ok(self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix.mul(&value),
                },
            ));
        }

        // Chronological DPLL uses the frequency table from the single
        // residual scan. Do not divide by weights or assume zero => UNSAT.
        let var = *active
            .iter()
            .max_by_key(|&&v| frequency[v])
            .expect("nonempty residual has an active variable");
        let child: Vec<_> = active.into_iter().filter(|&v| v != var).collect();
        let yes = self.count(assign(&residual, var as i32), child.clone(), depth + 1)?;
        let no = self.count(assign(&residual, -(var as i32)), child, depth + 1)?;
        let sum = self
            .weight(var, true)
            .mul(&yes.count)
            .add(&self.weight(var, false).mul(&no.count));
        Ok(self.save(
            key,
            Outcome {
                sat: yes.sat || no.sat,
                count: prefix.mul(&sum),
            },
        ))
    }
}

fn normalize(clauses: Vec<Clause>) -> Vec<Clause> {
    let mut result = Vec::new();
    for mut c in clauses {
        c.sort_unstable();
        c.dedup();
        if c.iter().any(|&lit| c.binary_search(&-lit).is_ok()) {
            continue;
        }
        result.push(c);
    }
    result.sort();
    result.dedup();
    result
}
fn assign(clauses: &[Clause], literal: i32) -> Vec<Clause> {
    clauses
        .iter()
        .filter(|c| !c.contains(&literal))
        .map(|c| c.iter().copied().filter(|&lit| lit != -literal).collect())
        .collect()
}
fn root(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        let p = parent[x];
        parent[x] = parent[p];
        x = parent[x];
    }
    x
}
fn split_components(
    clauses: &[Clause],
    scope: &[usize],
    variables: usize,
) -> Option<Vec<(Vec<Clause>, Vec<usize>)>> {
    let mut parent: Vec<_> = (0..=variables).collect();
    for c in clauses {
        let first = c[0].unsigned_abs() as usize;
        for &lit in &c[1..] {
            let a = root(&mut parent, first);
            let b = root(&mut parent, lit.unsigned_abs() as usize);
            parent[b] = a;
        }
    }
    // Most residuals stay connected. Avoid allocating and cloning a full
    // partition when no decomposition can be performed.
    let &first = scope.first()?;
    let first_root = root(&mut parent, first);
    if scope
        .iter()
        .all(|&var| root(&mut parent, var) == first_root)
    {
        return None;
    }
    let mut groups: BTreeMap<usize, (Vec<usize>, Vec<Clause>)> = BTreeMap::new();
    for &var in scope {
        groups
            .entry(root(&mut parent, var))
            .or_default()
            .0
            .push(var);
    }
    for c in clauses {
        let r = root(&mut parent, c[0].unsigned_abs() as usize);
        groups
            .get_mut(&r)
            .expect("residual variable belongs to scope")
            .1
            .push(c.clone());
    }
    Some(
        groups
            .into_values()
            .map(|(vars, cls)| (cls, vars))
            .collect(),
    )
}
