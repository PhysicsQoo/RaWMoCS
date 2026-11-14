//! Experimental exact, full complex weighted model counting over Q[i].
//! No projected-WMC or quantum-equivalence claims are made by this crate.

#[cfg(feature = "canonical-mul-research")]
use num_integer::Integer;
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
        // Exact zero is the additive identity. Skip two BigRational
        // additions (and their normalization) when an operand vanishes.
        if self.real.is_zero() && self.imag.is_zero() {
            return rhs.clone();
        }
        if rhs.real.is_zero() && rhs.imag.is_zero() {
            return self.clone();
        }
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

    /// Opt-in, research-only: canonical operands imply cross-cancelled products
    /// are already reduced. This is NEVER used by the public Gaussian::mul.
    #[cfg(feature = "canonical-mul-research")]
    fn mul_canonical_research(&self, rhs: &Self) -> Self {
        fn mul(a: &BigRational, b: &BigRational) -> BigRational {
            #[cfg(debug_assertions)]
            for value in [a, b] {
                let canonical = BigRational::new(value.numer().clone(), value.denom().clone());
                debug_assert_eq!(value.numer(), canonical.numer());
                debug_assert_eq!(value.denom(), canonical.denom());
            }
            let g1 = a.numer().gcd(b.denom());
            let g2 = a.denom().gcd(b.numer());
            let n = (a.numer() / &g1) * (b.numer() / &g2);
            let d = (a.denom() / &g2) * (b.denom() / &g1);
            BigRational::new_raw(n, d)
        }

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
        if self.imag.is_zero() {
            return Self::new(mul(&self.real, &rhs.real), mul(&self.real, &rhs.imag));
        }
        if rhs.imag.is_zero() {
            return Self::new(mul(&self.real, &rhs.real), mul(&self.imag, &rhs.real));
        }
        if self.real.is_zero() {
            return Self::new(-mul(&self.imag, &rhs.imag), mul(&self.imag, &rhs.real));
        }
        if rhs.real.is_zero() {
            return Self::new(-mul(&self.imag, &rhs.imag), mul(&self.real, &rhs.imag));
        }
        Self::new(
            mul(&self.real, &rhs.real) - mul(&self.imag, &rhs.imag),
            mul(&self.real, &rhs.imag) + mul(&self.imag, &rhs.real),
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
    /// Root CNF canonicalization before any key construction.
    /// Instrumentation perturbs wall time; not part of Key build or lookup.
    pub root_normalize_calls: u64,
    pub root_normalize_ns: u128,
    /// Recursive residual canonicalization before cache lookup.
    pub recursive_normalize_calls: u64,
    pub recursive_normalize_ns: u128,
    /// Number of lookups when the retained entry count is already at cap.
    /// Lookups still remain active after the insertion budget is exhausted.
    pub full_cache_lookup_calls: u64,
    /// Hits that occurred when the cache was already full, a subset of
    /// full_cache_lookup_calls and hit_clone_calls.
    pub full_cache_lookup_hits: u64,
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
    /// Entries rejected by the experimental observed-first-solve work proxy.
    #[cfg(feature = "cache-admission-research")]
    pub cache_work_proxy_rejections: u64,
    /// Number of decisions where an actual non-unit pivot was selected.
    #[cfg(feature = "branching-research")]
    pub pivot_decisions: u64,
    /// Min-fill deliberately falls back to frequency on oversized residuals.
    #[cfg(feature = "branching-research")]
    pub pivot_fallbacks: u64,
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
    #[cfg(feature = "cache-admission-research")]
    work_proxy_ratio: Option<u64>,
    #[cfg(feature = "branching-research")]
    pivot_policy: PivotPolicy,
    #[cfg(feature = "canonical-mul-research")]
    canonical_mul_research: bool,
    stats: Stats,
    cache: HashMap<Key, Outcome>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub nodes: u64,
    pub depth: usize,
    pub cache_entries: usize,
}

/// Independent *research-only* DPLL variable order choices. No alternate
/// choice affects default solver builds unless explicitly requested.
#[cfg(feature = "branching-research")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotPolicy {
    Frequency,
    ShortClause,
    MinFill,
}

#[cfg(feature = "branching-research")]
pub fn solve_with_pivot_policy(
    p: &Problem,
    limits: Limits,
    policy: PivotPolicy,
) -> Result<Report, SolveError> {
    solve_with_solver_policy(
        p,
        limits,
        None,
        0,
        None,
        policy,
        #[cfg(feature = "canonical-mul-research")]
        false,
    )
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
    solve_with_research_cache_threshold(p, limits, None, min_clauses, None)
}

/// Experimental first-solve effort admission, available only with the
/// cache-admission-research feature. The node count is observed AFTER a
/// residual has been solved; it cannot forecast future re-use.
/// This fixed ratio is a research hypothesis, not a calibrated cost model.
#[cfg(feature = "cache-admission-research")]
pub fn solve_with_observed_work_ratio(
    p: &Problem,
    limits: Limits,
    ratio: u64,
) -> Result<Report, SolveError> {
    assert!(ratio > 0, "research work proxy ratio must be positive");
    solve_with_research_cache_threshold(p, limits, None, 0, Some(ratio))
}

fn solve_with_optional_cache_payload_budget(
    p: &Problem,
    limits: Limits,
    cache_payload_budget: Option<CachePayloadBudget>,
) -> Result<Report, SolveError> {
    solve_with_research_cache_threshold(p, limits, cache_payload_budget, 0, None)
}

fn solve_with_research_cache_threshold(
    p: &Problem,
    limits: Limits,
    cache_payload_budget: Option<CachePayloadBudget>,
    min_cache_clauses: usize,
    work_proxy_ratio: Option<u64>,
) -> Result<Report, SolveError> {
    solve_with_solver_policy(
        p,
        limits,
        cache_payload_budget,
        min_cache_clauses,
        work_proxy_ratio,
        #[cfg(feature = "branching-research")]
        PivotPolicy::Frequency,
        #[cfg(feature = "canonical-mul-research")]
        false,
    )
}

fn solve_with_solver_policy(
    p: &Problem,
    limits: Limits,
    cache_payload_budget: Option<CachePayloadBudget>,
    min_cache_clauses: usize,
    work_proxy_ratio: Option<u64>,
    #[cfg(feature = "branching-research")] pivot_policy: PivotPolicy,
    #[cfg(feature = "canonical-mul-research")] canonical_mul_research: bool,
) -> Result<Report, SolveError> {
    #[cfg(not(feature = "cache-admission-research"))]
    let _ = (min_cache_clauses, work_proxy_ratio);
    let mut engine = Engine {
        p,
        budget: limits.nodes,
        depth_budget: limits.depth,
        cache_budget: limits.cache_entries,
        cache_payload_budget,
        #[cfg(feature = "cache-admission-research")]
        min_cache_clauses,
        #[cfg(feature = "cache-admission-research")]
        work_proxy_ratio,
        #[cfg(feature = "branching-research")]
        pivot_policy,
        #[cfg(feature = "canonical-mul-research")]
        canonical_mul_research,
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

/// Research-only Full WMC entrypoint. Clone and normalize all externally
/// supplied weights exactly once, before enabling the canonical-only
/// multiplication algorithm. The normal solver and public Gaussian::mul stay
/// unchanged; invalid zero denominators fail closed in this opt-in path.
#[cfg(feature = "canonical-mul-research")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalMulResearchError {
    ZeroDenominator {
        variable: usize,
        positive: bool,
        component: &'static str,
    },
    Solve(SolveError),
}

/// Research-only baseline with exactly the SAME external cloning,
/// denominator validation and canonicalization as the opt-in arithmetic arm.
/// Uses established, normalizing multiplication; never alters defaults.
#[cfg(feature = "canonical-mul-research")]
pub fn solve_with_normalized_input_research(
    problem: &Problem,
    limits: Limits,
) -> Result<Report, CanonicalMulResearchError> {
    let canonical = normalize_research_problem(problem)?;
    solve_with_limits(&canonical, limits).map_err(CanonicalMulResearchError::Solve)
}

/// Shared entry cost across both research arms; proof of field equality is
/// tested independently. Invalid denominator fails before any solve begins.
#[cfg(feature = "canonical-mul-research")]
fn normalize_research_problem(problem: &Problem) -> Result<Problem, CanonicalMulResearchError> {
    let mut canonical = problem.clone();
    for (idx, weights) in canonical.weights.iter_mut().enumerate() {
        for (positive, component, r) in [
            (true, "real", &mut weights.positive.real),
            (true, "imag", &mut weights.positive.imag),
            (false, "real", &mut weights.negative.real),
            (false, "imag", &mut weights.negative.imag),
        ] {
            if r.denom().is_zero() {
                return Err(CanonicalMulResearchError::ZeroDenominator {
                    variable: idx + 1,
                    positive,
                    component,
                });
            }
            *r = BigRational::new(r.numer().clone(), r.denom().clone());
        }
    }
    Ok(canonical)
}

/// Immutable, privately canonicalized problem for research-only repeated solves.
///
/// Construction clones the source problem, rejects zero denominators and
/// normalizes every external rational exactly once. The private field cannot
/// be replaced with an unchecked Problem by a caller; no mutable access is
/// exposed. All methods are OFF by default behind canonical-mul-research.
#[cfg(feature = "canonical-mul-research")]
#[derive(Clone, Debug)]
pub struct PreparedCanonicalProblem {
    canonical: Problem,
}

#[cfg(feature = "canonical-mul-research")]
impl PreparedCanonicalProblem {
    /// Preparation cost must be included when reporting end-to-end timings.
    pub fn prepare(problem: &Problem) -> Result<Self, CanonicalMulResearchError> {
        let canonical = normalize_research_problem(problem)?;
        Ok(Self { canonical })
    }

    /// Reuse the same verified canonical input without repeating preparation.
    /// Every solve creates a new isolated DPLL Engine and cache, so no search
    /// results leak between repeated solves.
    pub fn solve_with_limits(&self, limits: Limits) -> Result<Report, SolveError> {
        solve_with_solver_policy(
            &self.canonical,
            limits,
            None,
            0,
            None,
            #[cfg(feature = "branching-research")]
            PivotPolicy::Frequency,
            true,
        )
    }

    /// Logical object storage estimate for an immutable, privately-owned
    /// prepared input. Includes Vec capacities, inline structs and actual
    /// numerator/denominator bit payloads; EXCLUDES hidden BigInt limb slack,
    /// allocator metadata, fragmentation, stack, cache and solver RSS.
    /// This is neither an exact allocation measure nor a hard memory bound.
    pub fn estimated_retained_payload_bytes(&self) -> usize {
        let p = &self.canonical;
        let mut size = std::mem::size_of::<Problem>()
            .saturating_add(
                p.clauses
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Vec<i32>>()),
            )
            .saturating_add(
                p.weights
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Weights>()),
            );
        for clause in &p.clauses {
            size =
                size.saturating_add(clause.capacity().saturating_mul(std::mem::size_of::<i32>()));
        }
        for weights in &p.weights {
            for r in [
                &weights.positive.real,
                &weights.positive.imag,
                &weights.negative.real,
                &weights.negative.imag,
            ] {
                for bits in [r.numer().bits(), r.denom().bits()] {
                    let bytes = bits.saturating_add(7) / 8;
                    size = size.saturating_add(usize::try_from(bytes).unwrap_or(usize::MAX));
                }
            }
        }
        size
    }
}

/// The timer around this call MUST include its input normalization cost.
/// The returned count remains a canonical exact Gaussian for valid weights.
#[cfg(feature = "canonical-mul-research")]
pub fn solve_with_canonical_mul_research(
    problem: &Problem,
    limits: Limits,
) -> Result<Report, CanonicalMulResearchError> {
    let canonical = normalize_research_problem(problem)?;
    solve_with_solver_policy(
        &canonical,
        limits,
        None,
        0,
        None,
        #[cfg(feature = "branching-research")]
        PivotPolicy::Frequency,
        true,
    )
    .map_err(CanonicalMulResearchError::Solve)
}

fn rational_value_bits(r: &BigRational) -> u64 {
    r.numer().bits().saturating_add(r.denom().bits())
}

fn gaussian_value_bits(value: &Gaussian) -> u64 {
    rational_value_bits(&value.real).saturating_add(rational_value_bits(&value.imag))
}

impl Engine<'_> {
    #[inline]
    fn multiply(&self, a: &Gaussian, b: &Gaussian) -> Gaussian {
        #[cfg(feature = "canonical-mul-research")]
        if self.canonical_mul_research {
            return a.mul_canonical_research(b);
        }
        a.mul(b)
    }

    fn save(
        &mut self,
        key: Option<Key>,
        outcome: Outcome,
        #[cfg(feature = "cache-admission-research")] start_nodes: u64,
    ) -> Outcome {
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
                #[cfg(feature = "cache-admission-research")]
                if let Some(ratio) = self.work_proxy_ratio {
                    // These are observed first-solve node visits, not
                    // predicted future hits or machine-level CPU cycles.
                    let observed_nodes = self.stats.nodes.saturating_sub(start_nodes);
                    let value_words = bits.saturating_add(63) / 64;
                    let key_units = (scope as u64)
                        .saturating_add(clauses as u64)
                        .saturating_add(literals as u64)
                        .saturating_add(value_words);
                    let admissible = observed_nodes > 1
                        && observed_nodes.saturating_mul(8) >= key_units.saturating_mul(ratio);
                    if !admissible {
                        self.stats.cache_work_proxy_rejections =
                            self.stats.cache_work_proxy_rejections.saturating_add(1);
                        return outcome;
                    }
                }
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
        #[cfg(feature = "cache-admission-research")]
        let start_nodes = self.stats.nodes;
        self.stats.nodes += 1;
        // With caching disabled, do not clone a scope or duplicate residual
        // clauses into a Key that will never be looked up or stored.
        // The root may contain arbitrary, unsorted input clauses. Every recursive
        // call receives either an assigned canonical residual or a component
        // partition of one. Both preserve each clause's sorted, tautology-free
        // form; only the outer clause ordering and duplicates can change.
        #[cfg(feature = "cache-cost-profile")]
        let normalize_start = Instant::now();
        let mut residual = if depth == 0 {
            normalize(clauses)
        } else {
            normalize_child(clauses)
        };
        #[cfg(feature = "cache-cost-profile")]
        if depth == 0 {
            self.stats.cache_cost.root_normalize_calls += 1;
            self.stats.cache_cost.root_normalize_ns += normalize_start.elapsed().as_nanos();
        } else {
            self.stats.cache_cost.recursive_normalize_calls += 1;
            self.stats.cache_cost.recursive_normalize_ns += normalize_start.elapsed().as_nanos();
        }
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
            let cache_full_before_lookup = self.cache.len() == self.cache_budget;
            #[cfg(feature = "cache-cost-profile")]
            let lookup_start = Instant::now();
            let cached = self.cache.get(&key);
            #[cfg(feature = "cache-cost-profile")]
            {
                self.stats.cache_cost.lookup_calls += 1;
                self.stats.cache_cost.lookup_ns += lookup_start.elapsed().as_nanos();
                if cache_full_before_lookup {
                    self.stats.cache_cost.full_cache_lookup_calls += 1;
                    if cached.is_some() {
                        self.stats.cache_cost.full_cache_lookup_hits += 1;
                    }
                }
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
                return Ok(self.save(
                    key,
                    Outcome::unsat(),
                    #[cfg(feature = "cache-admission-research")]
                    start_nodes,
                ));
            }
            let Some(lit) = residual.iter().find(|c| c.len() == 1).map(|c| c[0]) else {
                break;
            };
            let var = lit.unsigned_abs() as usize;
            prefix = self.multiply(&prefix, self.weight(var, lit > 0));
            remaining.retain(|&v| v != var);
            residual = assign(&residual, lit);
        }

        // When unit propagation resolves every clause, all remaining
        // variables are free. Avoid allocating any residual frequency table.
        if residual.is_empty() {
            for var in remaining {
                let w = &self.p.weights[var - 1];
                prefix = self.multiply(&prefix, &w.positive.add(&w.negative));
            }
            return Ok(self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix,
                },
                #[cfg(feature = "cache-admission-research")]
                start_nodes,
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
                prefix = self.multiply(&prefix, &w.positive.add(&w.negative));
            }
        }

        if let Some(pieces) = split_components(&residual, &active, self.p.variables) {
            self.stats.decompositions += 1;
            let mut value = Gaussian::one();
            for (c, s) in pieces {
                let part = self.count(c, s, depth + 1)?;
                if !part.sat {
                    return Ok(self.save(
                        key,
                        Outcome::unsat(),
                        #[cfg(feature = "cache-admission-research")]
                        start_nodes,
                    ));
                }
                value = self.multiply(&value, &part.count);
            }
            return Ok(self.save(
                key,
                Outcome {
                    sat: true,
                    count: self.multiply(&prefix, &value),
                },
                #[cfg(feature = "cache-admission-research")]
                start_nodes,
            ));
        }

        // Chronological DPLL uses the frequency table from the single
        // residual scan. Do not divide by weights or assume zero => UNSAT.
        #[cfg(feature = "branching-research")]
        let var = {
            self.stats.pivot_decisions += 1;
            match self.pivot_policy {
                PivotPolicy::Frequency => frequency_pivot(&active, &frequency),
                PivotPolicy::ShortClause => short_clause_pivot(&residual, &active, &frequency),
                PivotPolicy::MinFill => {
                    if active.len() > 64 || residual.iter().any(|c| c.len() > 16) {
                        self.stats.pivot_fallbacks += 1;
                        frequency_pivot(&active, &frequency)
                    } else {
                        min_fill_pivot(&residual, &active, &frequency)
                    }
                }
            }
        };
        #[cfg(not(feature = "branching-research"))]
        let var = *active
            .iter()
            .max_by_key(|&&v| frequency[v])
            .expect("nonempty residual has an active variable");
        let child: Vec<_> = active.into_iter().filter(|&v| v != var).collect();
        let yes = self.count(assign(&residual, var as i32), child.clone(), depth + 1)?;
        let no = self.count(assign(&residual, -(var as i32)), child, depth + 1)?;
        let yes_weighted = self.multiply(self.weight(var, true), &yes.count);
        let no_weighted = self.multiply(self.weight(var, false), &no.count);
        let sum = yes_weighted.add(&no_weighted);
        Ok(self.save(
            key,
            Outcome {
                sat: yes.sat || no.sat,
                count: self.multiply(&prefix, &sum),
            },
            #[cfg(feature = "cache-admission-research")]
            start_nodes,
        ))
    }
}

/// Preserve the original last-variable tie break of Iterator::max_by_key.
#[cfg(feature = "branching-research")]
fn frequency_pivot(active: &[usize], frequency: &[usize]) -> usize {
    *active
        .iter()
        .max_by_key(|&&v| frequency[v])
        .expect("nonempty residual has an active variable")
}

/// Integer Jeroslow-Wang-style short-clause preference, with ordinary
/// frequency as a deterministic second key and original last-tie semantics.
#[cfg(feature = "branching-research")]
fn short_clause_pivot(residual: &[Clause], active: &[usize], frequency: &[usize]) -> usize {
    let mut scores = vec![0u64; frequency.len()];
    for clause in residual {
        let weight = 1u64 << (8usize.saturating_sub(clause.len().min(8)));
        for &lit in clause {
            let var = lit.unsigned_abs() as usize;
            scores[var] = scores[var].saturating_add(weight);
        }
    }
    *active
        .iter()
        .max_by_key(|&&v| (scores[v], frequency[v]))
        .expect("nonempty residual has an active variable")
}

/// A deliberately bounded, explicitly expensive primal-graph min-fill
/// heuristic. Call only for <=64 active vars and clauses of length <=16.
/// Among equal missing edges use frequency and the original tie break.
#[cfg(feature = "branching-research")]
fn min_fill_pivot(residual: &[Clause], active: &[usize], frequency: &[usize]) -> usize {
    let size = active.len();
    let mut edges = vec![vec![false; size]; size];
    for clause in residual {
        let positions: Vec<_> = clause
            .iter()
            .map(|lit| {
                active
                    .iter()
                    .position(|&v| v == lit.unsigned_abs() as usize)
                    .expect("every non-free literal has an active variable")
            })
            .collect();
        for (index, &a) in positions.iter().enumerate() {
            for &b in positions.iter().skip(index + 1) {
                edges[a][b] = true;
                edges[b][a] = true;
            }
        }
    }
    let mut best = 0usize;
    let mut best_score = (std::cmp::Reverse(usize::MAX), 0usize);
    for (index, &var) in active.iter().enumerate() {
        let neighbors: Vec<_> = (0..size).filter(|&j| edges[index][j]).collect();
        let mut missing = 0usize;
        for (position, &a) in neighbors.iter().enumerate() {
            for &b in neighbors.iter().skip(position + 1) {
                if !edges[a][b] {
                    missing += 1;
                }
            }
        }
        let score = (std::cmp::Reverse(missing), frequency[var]);
        if score >= best_score {
            best = var;
            best_score = score;
        }
    }
    best
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
    normalize_child(result)
}

// Only called with clauses derived from normalize(), assign(), or a
// split_components() partition. Assignment removes literals but never changes
// their relative order or introduces a tautology; partitions copy whole clauses.
// Do not repeat per-clause sorting, de-duplication, and tautology checks on
// every DPLL node. Re-canonicalize the clause collection for exact cache keys.
fn normalize_child(mut clauses: Vec<Clause>) -> Vec<Clause> {
    clauses.sort();
    clauses.dedup();
    clauses
}
/// Fast residual assignment for normalized CNFs. Preserve clause order and
/// the exact Boolean substitution; prioritize the common two-literal case.
/// Root normalization guarantees clauses are deduplicated and tautology-free.
fn assign(clauses: &[Clause], literal: i32) -> Vec<Clause> {
    let opposite = -literal;
    let mut child = Vec::with_capacity(clauses.len());
    for clause in clauses {
        if clause.len() == 2 {
            let a = clause[0];
            let b = clause[1];
            if a == literal || b == literal {
                continue;
            }
            if a == opposite {
                child.push(vec![b]);
            } else if b == opposite {
                child.push(vec![a]);
            } else {
                child.push(clause.clone());
            }
        } else if clause.contains(&literal) {
            continue;
        } else if clause.contains(&opposite) {
            child.push(
                clause
                    .iter()
                    .copied()
                    .filter(|&lit| lit != opposite)
                    .collect(),
            );
        } else {
            child.push(clause.clone());
        }
    }
    child
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

#[cfg(test)]
mod assignment_substitution_tests {
    use super::{assign, normalize, Clause};

    // Independent reference preserves the original literal-level Boolean
    // assignment semantics rather than reusing the optimized branches.
    fn reference(clauses: &[Clause], selected: i32) -> Vec<Clause> {
        clauses
            .iter()
            .filter(|c| !c.contains(&selected))
            .map(|c| c.iter().copied().filter(|&lit| lit != -selected).collect())
            .collect()
    }

    #[test]
    fn binary_assignment_matches_reference_on_all_small_literal_pairs() {
        for n in 1_i32..=5 {
            let literals: Vec<i32> = (-n..=n).filter(|&x| x != 0).collect();
            let mut clauses = vec![vec![]];
            for &a in &literals {
                clauses.push(vec![a]);
                for &b in &literals {
                    clauses.push(vec![a, b]);
                }
            }
            for &a in &literals {
                for &b in &literals {
                    for &c in &literals {
                        if n <= 3 {
                            clauses.push(vec![a, b, c]);
                        }
                    }
                }
            }
            // The solver calls assign only after canonicalization.
            let clauses = normalize(clauses);
            for selected in literals {
                assert_eq!(
                    assign(&clauses, selected),
                    reference(&clauses, selected),
                    "n={n}, assignment={selected}"
                );
            }
        }
    }

    #[test]
    fn assignment_matches_reference_after_recursive_residuals() {
        let root = normalize(vec![
            vec![-1, 2],
            vec![1, -2, 3],
            vec![-3, 4],
            vec![2, 4],
            vec![-2, -4],
            vec![1, 5],
            vec![4, -5],
        ]);
        for initial in [-5_i32, -4, -3, -2, -1, 1, 2, 3, 4, 5] {
            let residual = super::normalize_child(reference(&root, initial));
            for next in [-5_i32, -4, -3, -2, -1, 1, 2, 3, 4, 5] {
                assert_eq!(assign(&residual, next), reference(&residual, next));
            }
        }
    }
}

#[cfg(test)]
mod residual_canonicalization_tests {
    use super::{assign, normalize, normalize_child, split_components, Clause};

    #[test]
    fn recursive_residuals_match_full_normalization_under_all_binary_assignments() {
        // Includes a tautology, duplicate literals, repeated clauses,
        // mixed signs, and branches producing empty/duplicate clauses.
        let raw: Vec<Clause> = vec![
            vec![1, 2],
            vec![1, 3],
            vec![-1, 3],
            vec![-1, -3],
            vec![2, -3],
            vec![2, 3],
            vec![-2, 3],
            vec![-2, -3],
            vec![1, -1, 4],
            vec![4, 4],
            vec![-4],
            vec![3, 4],
            vec![4, 3],
            vec![-1, 2, 4],
        ];
        let mut level = vec![normalize(raw)];
        for var in 1_i32..=4 {
            let mut next = Vec::new();
            for parent in level {
                for literal in [var, -var] {
                    let assigned = assign(&parent, literal);
                    assert!(assigned
                        .iter()
                        .all(|clause| { clause.windows(2).all(|pair| pair[0] < pair[1]) }));
                    let reference = normalize(assigned.clone());
                    let fast = normalize_child(assigned);
                    assert_eq!(fast, reference, "failed for assignment {literal}");
                    next.push(fast);
                }
            }
            level = next;
        }
    }

    #[test]
    fn partitioned_residuals_match_full_normalization() {
        let clauses = normalize(vec![
            vec![1, 2],
            vec![-1, 2],
            vec![3, 4],
            vec![-3, 4],
            vec![5, 6],
            vec![-5, -6],
            vec![3, 4],
        ]);
        let parts = split_components(&clauses, &[1, 2, 3, 4, 5, 6], 6)
            .expect("fixture must split into three connected components");
        assert_eq!(parts.len(), 3);
        for (component, _) in parts {
            assert_eq!(normalize_child(component.clone()), normalize(component));
        }
    }
}
