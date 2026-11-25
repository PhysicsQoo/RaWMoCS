//! Minimal production port of the original MPFR chronological DPLL control.
//! Research source: 379b47242cbd9b4a1e597afc6377cbaf0ac13bce,
//! benchmarks/mpfr_research/src/main.rs (Engine::count, normalize,
//! assign, split_components). No experimental cache or branching switches.
use crate::{Clause, Outcome, Problem, C};
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

#[derive(Hash, Eq, PartialEq)]
struct Key {
    scope: Vec<usize>,
    clauses: Vec<Clause>,
}
struct Engine<'a> {
    p: &'a Problem,
    bits: u32,
    deadline: Instant,
    cache: HashMap<Key, Outcome>,
    cache_items: usize,
}
fn normalize(mut clauses: Vec<Clause>) -> Vec<Clause> {
    for clause in &mut clauses {
        clause.sort_unstable();
        clause.dedup();
    }
    clauses.retain(|c| !c.iter().any(|&l| c.binary_search(&-l).is_ok()));
    clauses.sort();
    clauses.dedup();
    clauses
}
fn assign(clauses: &[Clause], lit: i32) -> Vec<Clause> {
    clauses
        .iter()
        .filter(|c| !c.contains(&lit))
        .map(|c| c.iter().copied().filter(|&x| x != -lit).collect())
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
    n: usize,
) -> Vec<(Vec<usize>, Vec<Clause>)> {
    let mut parent: Vec<_> = (0..=n).collect();
    for c in clauses {
        let first = c[0].unsigned_abs() as usize;
        for &lit in &c[1..] {
            let a = root(&mut parent, first);
            let b = root(&mut parent, lit.unsigned_abs() as usize);
            parent[b] = a;
        }
    }
    let mut groups: BTreeMap<usize, (Vec<usize>, Vec<Clause>)> = BTreeMap::new();
    for &v in scope {
        groups.entry(root(&mut parent, v)).or_default().0.push(v);
    }
    if groups.len() <= 1 {
        return Vec::new();
    }
    for c in clauses {
        let r = root(&mut parent, c[0].unsigned_abs() as usize);
        groups.entry(r).or_default().1.push(c.clone());
    }
    groups.into_values().collect()
}
impl Engine<'_> {
    fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("general deadline".into());
        }
        Ok(())
    }
    fn weight(&self, v: usize, positive: bool) -> &C {
        let w = &self.p.weights[v - 1];
        if positive {
            &w.positive
        } else {
            &w.negative
        }
    }
    fn save(&mut self, key: Key, out: Outcome) -> Result<Outcome, String> {
        if !out.count.re.is_finite() || !out.count.im.is_finite() {
            return Err("nonfinite general".into());
        }
        let items = key.scope.len()
            + key.clauses.len()
            + key.clauses.iter().map(Vec::len).sum::<usize>();
        // Cache admission is optional: exhaustion never stops search.
        if self.cache.len() < 10_000 && self.cache_items + items <= 400_000 {
            self.cache_items += items;
            self.cache.insert(key, out.clone());
        }
        Ok(out)
    }
    fn count(
        &mut self,
        clauses: Vec<Clause>,
        scope: Vec<usize>,
        depth: usize,
    ) -> Result<Outcome, String> {
        self.check()?;
        // Explicit stack resource guard; not a variable-count admission rule.
        if depth >= 512 {
            return Err("general stack limit".into());
        }
        let mut residual = normalize(clauses);
        let key = Key {
            scope: scope.clone(),
            clauses: residual.clone(),
        };
        if let Some(hit) = self.cache.get(&key) {
            return Ok(hit.clone());
        }
        let mut remaining = scope;
        let mut prefix = C::one(self.bits);
        loop {
            self.check()?;
            if residual.iter().any(Vec::is_empty) {
                return self.save(key, Outcome::unsat(self.bits));
            }
            let Some(lit) = residual.iter().find(|c| c.len() == 1).map(|c| c[0]) else {
                break;
            };
            let v = lit.unsigned_abs() as usize;
            prefix = prefix.mul(self.weight(v, lit > 0), self.bits);
            remaining.retain(|&x| x != v);
            residual = assign(&residual, lit);
        }
        let mut frequency = vec![0_usize; self.p.n + 1];
        for clause in &residual {
            for &lit in clause {
                frequency[lit.unsigned_abs() as usize] += 1;
            }
        }
        let mut active = Vec::new();
        for v in remaining {
            if frequency[v] == 0 {
                let w = &self.p.weights[v - 1];
                prefix = prefix.mul(&w.positive.add(&w.negative, self.bits), self.bits);
            } else {
                active.push(v);
            }
        }
        if residual.is_empty() {
            return self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix,
                },
            );
        }
        let parts = components(&residual, &active, self.p.n);
        if !parts.is_empty() {
            let mut value = C::one(self.bits);
            for (scope, clauses) in parts {
                let part = self.count(clauses, scope, depth + 1)?;
                if !part.sat {
                    return self.save(key, Outcome::unsat(self.bits));
                }
                value = value.mul(&part.count, self.bits);
            }
            return self.save(
                key,
                Outcome {
                    sat: true,
                    count: prefix.mul(&value, self.bits),
                },
            );
        }
        let v = *active
            .iter()
            .max_by_key(|&&v| frequency[v])
            .ok_or("no active pivot")?;
        let child: Vec<_> = active.into_iter().filter(|&x| x != v).collect();
        let yes = self.count(assign(&residual, v as i32), child.clone(), depth + 1)?;
        let no = self.count(assign(&residual, -(v as i32)), child, depth + 1)?;
        let y = self.weight(v, true).mul(&yes.count, self.bits);
        let n = self.weight(v, false).mul(&no.count, self.bits);
        self.save(
            key,
            Outcome {
                sat: yes.sat || no.sat,
                count: prefix.mul(&y.add(&n, self.bits), self.bits),
            },
        )
    }
}
pub(crate) fn solve(p: &Problem, bits: u32, deadline: Instant) -> Result<Outcome, String> {
    let mut engine = Engine {
        p,
        bits,
        deadline,
        cache: HashMap::new(),
        cache_items: 0,
    };
    engine.count(p.clauses.clone(), (1..=p.n).collect(), 0)
}
