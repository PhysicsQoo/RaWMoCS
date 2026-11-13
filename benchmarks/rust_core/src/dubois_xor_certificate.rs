//! Independent, source-checked XOR refutation of SATLIB Dubois20.
//!
//! The original DIMACS file is the only input. Its 160 three-literal clauses
//! form 40 consecutive four-clause parity groups. In each group, the four
//! forbidden Boolean assignments have the same parity, so satisfying the
//! group is equivalent to the opposite XOR equation. Every one of the 60
//! variables appears twice among those equations, while the XOR of all
//! right-hand sides is one. Their sum is 0=1: UNSAT for *any* WMC weights.
//!
//! This is a transparent algebraic certificate, NOT a DRAT/LRAT artifact,
//! an external checker, a measured default-solver runtime, or an unknown
//! resource claim. Validate every group before using the certificate.

use crate::satlib_provenance::original_dubois20_clauses;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XorUnsatCertificate {
    pub variables: usize,
    pub xor_equations: usize,
    pub rhs_xor: u8,
}

pub fn verify_dubois20_unsat() -> Result<XorUnsatCertificate, String> {
    let (variables, clauses) = original_dubois20_clauses();
    verify_original_xor_refutation(variables, &clauses)
}

fn verify_original_xor_refutation(
    variables: usize,
    clauses: &[Vec<i32>],
) -> Result<XorUnsatCertificate, String> {
    if variables != 60 || clauses.len() != 160 {
        return Err("source-pinned Dubois20 requires exactly 60 vars and 160 clauses".into());
    }
    let mut occurrences = vec![0_usize; variables + 1];
    let mut rhs_xor = 0_u8;

    for (group_index, group) in clauses.chunks_exact(4).enumerate() {
        if group.iter().any(|clause| clause.len() != 3) {
            return Err(format!(
                "group {group_index}: each clause needs three literals"
            ));
        }
        let mut triple: Vec<usize> = group[0]
            .iter()
            .map(|lit| lit.unsigned_abs() as usize)
            .collect();
        triple.sort_unstable();
        triple.dedup();
        if triple.len() != 3 || triple[0] == 0 || triple[2] > variables {
            return Err(format!(
                "group {group_index}: invalid three-variable triple"
            ));
        }

        let mut forbidden_masks = BTreeSet::new();
        let mut forbidden_parity = None;
        for clause in group {
            let mut clause_variables: Vec<usize> = clause
                .iter()
                .map(|lit| lit.unsigned_abs() as usize)
                .collect();
            clause_variables.sort_unstable();
            if clause_variables != triple {
                return Err(format!("group {group_index}: inconsistent variable triple"));
            }

            // Each 3-literal OR clause is false at exactly one assignment.
            // A negative literal is false when its variable is True.
            let mut forbidden = 0_u8;
            for &lit in clause {
                if lit < 0 {
                    let var = lit.unsigned_abs() as usize;
                    let bit = triple
                        .iter()
                        .position(|&candidate| candidate == var)
                        .ok_or_else(|| format!("group {group_index}: missing variable"))?;
                    forbidden |= 1 << bit;
                }
            }
            let parity = (forbidden.count_ones() & 1) as u8;
            if let Some(previous) = forbidden_parity {
                if previous != parity {
                    return Err(format!("group {group_index}: inconsistent XOR parity"));
                }
            } else {
                forbidden_parity = Some(parity);
            }
            if !forbidden_masks.insert(forbidden) {
                return Err(format!(
                    "group {group_index}: repeated forbidden assignment"
                ));
            }
        }
        if forbidden_masks.len() != 4 {
            return Err(format!("group {group_index}: incomplete parity half-space"));
        }
        // The group forbids all four assignments of the opposite parity.
        rhs_xor ^= forbidden_parity.expect("nonempty source-pinned group") ^ 1;
        for variable in triple {
            occurrences[variable] += 1;
        }
    }

    if occurrences.iter().skip(1).any(|&n| n != 2) {
        return Err("each original variable must occur exactly twice in XOR equations".into());
    }
    if rhs_xor != 1 {
        return Err("XOR of all right-hand sides must be odd (0 = 1)".into());
    }
    Ok(XorUnsatCertificate {
        variables,
        xor_equations: clauses.len() / 4,
        rhs_xor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_source_dubois20_xor_refutation_proves_unsat() {
        let certificate = verify_dubois20_unsat().expect("all source clauses justify XOR proof");
        assert_eq!(
            certificate,
            XorUnsatCertificate {
                variables: 60,
                xor_equations: 40,
                rhs_xor: 1,
            }
        );
    }

    #[test]
    fn changing_a_clause_cannot_reuse_the_original_certificate() {
        let (variables, mut clauses) = original_dubois20_clauses();
        clauses[0][0] = -clauses[0][0];
        assert!(verify_original_xor_refutation(variables, &clauses).is_err());
    }

    #[test]
    fn missing_or_duplicated_source_clause_invalidates_the_certificate() {
        let (variables, mut clauses) = original_dubois20_clauses();
        clauses.remove(0);
        assert!(verify_original_xor_refutation(variables, &clauses).is_err());
        let (variables, mut clauses) = original_dubois20_clauses();
        clauses[1] = clauses[0].clone();
        assert!(verify_original_xor_refutation(variables, &clauses).is_err());
    }
}
