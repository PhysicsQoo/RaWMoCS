//! Experimental exact, full complex weighted model counting over Q[i].
//! No projected-WMC or quantum-equivalence claims are made by this crate.

use num_rational::BigRational;
use num_traits::{One, Zero};
use std::collections::{BTreeMap, HashMap};

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
                if lit == 0 || i64::from(lit).abs() as usize > variables {
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
    NodeBudgetExceeded,
}
impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SolveError {}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub nodes: u64,
    pub cache_hits: u64,
    pub decompositions: u64,
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
    stats: Stats,
    cache: HashMap<Key, Outcome>,
}

pub fn solve_with_budget(p: &Problem, budget: u64) -> Result<Report, SolveError> {
    let mut engine = Engine {
        p,
        budget,
        stats: Stats::default(),
        cache: HashMap::new(),
    };
    let answer = engine.count(p.clauses.clone(), (1..=p.variables).collect())?;
    Ok(Report {
        satisfiable: answer.sat,
        count: answer.count,
        stats: engine.stats,
    })
}
pub fn solve(p: &Problem) -> Result<Report, SolveError> {
    solve_with_budget(p, 100_000)
}

impl Engine<'_> {
    fn save(&mut self, key: Key, outcome: Outcome) -> Outcome {
        self.cache.insert(key, outcome.clone());
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
    fn count(&mut self, clauses: Vec<Clause>, scope: Vec<usize>) -> Result<Outcome, SolveError> {
        if self.stats.nodes >= self.budget {
            return Err(SolveError::NodeBudgetExceeded);
        }
        self.stats.nodes += 1;
        let key = Key {
            scope: scope.clone(),
            clauses: normalize(clauses),
        };
        if let Some(cached) = self.cache.get(&key) {
            self.stats.cache_hits += 1;
            return Ok(cached.clone());
        }

        let mut residual = key.clauses.clone();
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

        // Each free variable contributes w(x) + w(!x) exactly once.
        let mut appears = vec![false; self.p.variables + 1];
        for c in &residual {
            for &lit in c {
                appears[lit.unsigned_abs() as usize] = true;
            }
        }
        let mut active = Vec::new();
        for var in remaining {
            if appears[var] {
                active.push(var);
            } else {
                let w = &self.p.weights[var - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative));
            }
        }
        if residual.is_empty() {
            return Ok(self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix,
                },
            ));
        }

        let pieces = components(&residual, &active, self.p.variables);
        if pieces.len() > 1 {
            self.stats.decompositions += 1;
            let mut value = Gaussian::one();
            for (c, s) in pieces {
                let part = self.count(c, s)?;
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

        // Chronological DPLL. Do not divide by weights or assume zero => UNSAT.
        let mut frequency = vec![0usize; self.p.variables + 1];
        for c in &residual {
            for &lit in c {
                frequency[lit.unsigned_abs() as usize] += 1;
            }
        }
        let var = *active
            .iter()
            .max_by_key(|&&v| frequency[v])
            .expect("nonempty residual has an active variable");
        let child: Vec<_> = active.into_iter().filter(|&v| v != var).collect();
        let yes = self.count(assign(&residual, var as i32), child.clone())?;
        let no = self.count(assign(&residual, -(var as i32)), child)?;
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
fn components(
    clauses: &[Clause],
    scope: &[usize],
    variables: usize,
) -> Vec<(Vec<Clause>, Vec<usize>)> {
    let mut parent: Vec<_> = (0..=variables).collect();
    for c in clauses {
        let first = c[0].unsigned_abs() as usize;
        for &lit in &c[1..] {
            let a = root(&mut parent, first);
            let b = root(&mut parent, lit.unsigned_abs() as usize);
            parent[b] = a;
        }
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
    groups
        .into_values()
        .map(|(vars, cls)| (cls, vars))
        .collect()
}
