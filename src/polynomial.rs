//! Port of OmniX's polynomial canonicalization rules, not an erasure code.
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Term { pub coefficient: f64, pub exponents: Vec<i64> }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error { InconsistentVariableCount, NonFiniteCoefficient }

#[derive(Clone, Debug, PartialEq)]
pub struct Canonical { pub terms: Vec<Term>, pub form: String, pub hash: String }

/// Combines like terms, drops near-zero sums, and sorts exponent vectors.
/// Rust serialization is versioned separately; hashes need not match OmniX C++.
pub fn canonicalize(input: &[Term]) -> Result<Canonical, Error> {
    let variables = input.first().map(|t| t.exponents.len());
    let mut combined = BTreeMap::<Vec<i64>, f64>::new();
    for term in input {
        if Some(term.exponents.len()) != variables { return Err(Error::InconsistentVariableCount); }
        if !term.coefficient.is_finite() { return Err(Error::NonFiniteCoefficient); }
        let sum = combined.entry(term.exponents.clone()).or_default();
        *sum += term.coefficient;
        if !sum.is_finite() { return Err(Error::NonFiniteCoefficient); }
    }
    let terms: Vec<Term> = combined.into_iter().filter(|(_, c)| c.abs() >= 1e-9)
        .map(|(exponents, coefficient)| Term { coefficient, exponents }).collect();
    let form = terms.iter().map(|t| format!("coeff:{:.12e}|exp:{:?}", t.coefficient, t.exponents))
        .collect::<Vec<_>>().join(";");
    let hash = format!("{:x}", Sha256::digest(form.as_bytes()));
    Ok(Canonical { terms, form, hash })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_and_canceled_terms() {
        let term = |c| Term { coefficient: c, exponents: vec![1] };
        assert_eq!(canonicalize(&[term(1.0), term(1.0)]).unwrap().hash, canonicalize(&[term(2.0)]).unwrap().hash);
        assert_eq!(canonicalize(&[term(1.0), term(-1.0)]).unwrap().hash, canonicalize(&[]).unwrap().hash);
    }
    #[test]
    fn rejects_mixed_dimensions() {
        assert_eq!(canonicalize(&[Term { coefficient: 1.0, exponents: vec![1] }, Term { coefficient: 2.0, exponents: vec![1, 2] }]), Err(Error::InconsistentVariableCount));
    }
}
