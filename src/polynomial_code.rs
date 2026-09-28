//! Systematic 4+2 GF(2^8) parity with rows [1,1,1,1] and [1,2,3,4].
//! This is an RS-family MDS code, not symbolic polynomial canonicalization.
use crate::storage::{BLOCK_SIZE, DATA_SHARDS, PARITY_SHARDS};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct Stripe {
    pub shards: Vec<Vec<u8>>,
    pub original_len: usize,
    pub shard_hashes: Vec<[u8; 32]>,
}

fn mul(mut a: u8, mut b: u8) -> u8 {
    let mut out = 0;
    while b != 0 {
        if b & 1 != 0 { out ^= a; }
        let carry = a & 0x80 != 0;
        a <<= 1;
        if carry { a ^= 0x1d; } // x^8+x^4+x^3+x^2+1
        b >>= 1;
    }
    out
}

fn pow(mut x: u8, mut n: u16) -> u8 {
    let mut out = 1;
    while n != 0 {
        if n & 1 != 0 { out = mul(out, x); }
        x = mul(x, x); n >>= 1;
    }
    out
}

fn row(index: usize) -> [u8; 4] {
    if index < 4 {
        let mut r = [0; 4]; r[index] = 1; r
    } else if index == 4 { [1, 1, 1, 1] } else { [1, 2, 3, 4] }
}

fn inverse(mut a: [[u8; 8]; 4]) -> Result<[[u8; 4]; 4], String> {
    for col in 0..4 {
        let pivot = (col..4).find(|&r| a[r][col] != 0).ok_or("singular share set")?;
        a.swap(col, pivot);
        let scale = pow(a[col][col], 254);
        for j in 0..8 { a[col][j] = mul(a[col][j], scale); }
        for r in 0..4 {
            if r == col { continue; }
            let factor = a[r][col];
            for j in 0..8 { a[r][j] ^= mul(factor, a[col][j]); }
        }
    }
    let mut result = [[0; 4]; 4];
    for i in 0..4 { result[i].copy_from_slice(&a[i][4..8]); }
    Ok(result)
}

pub fn encode_stripe(bytes: &[u8]) -> Result<Stripe, String> {
    if bytes.len() > DATA_SHARDS * BLOCK_SIZE { return Err("stripe exceeds 1 MiB".into()); }
    let mut shards = vec![vec![0u8; BLOCK_SIZE]; DATA_SHARDS + PARITY_SHARDS];
    for (dst, src) in shards[..DATA_SHARDS].iter_mut().zip(bytes.chunks(BLOCK_SIZE)) {
        dst[..src.len()].copy_from_slice(src);
    }
    encode_parity_in_place(&mut shards)?;
    let shard_hashes = shards.iter().map(|s| Sha256::digest(s).into()).collect();
    Ok(Stripe { shards, original_len: bytes.len(), shard_hashes })
}

pub fn encode_parity_in_place(shards: &mut [Vec<u8>]) -> Result<(), String> {
    if shards.len() != 6 || shards.iter().any(|s| s.len() != BLOCK_SIZE) { return Err("invalid shard shape".into()); }
    for j in 0..BLOCK_SIZE {
        let mut p0 = 0; let mut p1 = 0;
        for i in 0..DATA_SHARDS {
            let value = shards[i][j];
            p0 ^= value;
            p1 ^= mul((i + 1) as u8, value);
        }
        shards[4][j] = p0; shards[5][j] = p1;
    }
    Ok(())
}

pub fn reconstruct(mut shares: Vec<Option<Vec<u8>>>, original_len: usize, hashes: &[[u8; 32]]) -> Result<Vec<u8>, String> {
    if shares.len() != 6 || hashes.len() != 6 || original_len > DATA_SHARDS * BLOCK_SIZE { return Err("invalid stripe manifest".into()); }
    for (share, hash) in shares.iter_mut().zip(hashes) {
        if let Some(data) = share {
            if data.len() != BLOCK_SIZE || Sha256::digest(data.as_slice()).as_slice() != hash { *share = None; }
        }
    }
    let data = reconstruct_data_unchecked(shares)?;
    for (share, hash) in data.iter().zip(hashes.iter()) {
        if Sha256::digest(share).as_slice() != hash { return Err("reconstructed data hash mismatch".into()); }
    }
    let mut output = data.into_iter().flatten().collect::<Vec<_>>();
    output.truncate(original_len);
    Ok(output)
}

/// Benchmarkable recovery math; caller must validate share hashes first.
pub fn reconstruct_data_unchecked(shares: Vec<Option<Vec<u8>>>) -> Result<Vec<Vec<u8>>, String> {
    if shares.len() != 6 || shares.iter().flatten().any(|s| s.len() != BLOCK_SIZE) { return Err("invalid shard shape".into()); }
    let missing_data: Vec<usize> = (0..4).filter(|&i| shares[i].is_none()).collect();
    let mut data: Vec<Vec<u8>> = (0..4).map(|i| shares[i].clone().unwrap_or_else(|| vec![0; BLOCK_SIZE])).collect();
    if missing_data.len() == 1 && shares[4].is_some() {
        let target = missing_data[0];
        for j in 0..BLOCK_SIZE {
            let mut value = shares[4].as_ref().unwrap()[j];
            for (i, share) in data.iter().enumerate() { if i != target { value ^= share[j]; } }
            data[target][j] = value;
        }
    } else if missing_data.len() == 2 && shares[4].is_some() && shares[5].is_some() {
        let a = missing_data[0]; let b = missing_data[1];
        let divisor = pow((a as u8 + 1) ^ (b as u8 + 1), 254);
        for j in 0..BLOCK_SIZE {
            let mut sum = shares[4].as_ref().unwrap()[j];
            let mut weighted = shares[5].as_ref().unwrap()[j];
            for (i, share) in data.iter().enumerate() {
                if i != a && i != b { sum ^= share[j]; weighted ^= mul(i as u8 + 1, share[j]); }
            }
            let value_a = mul(divisor, weighted ^ mul(b as u8 + 1, sum));
            data[a][j] = value_a; data[b][j] = sum ^ value_a;
        }
    } else if !missing_data.is_empty() {
        let available: Vec<usize> = (0..6).filter(|&i| shares[i].is_some()).take(4).collect();
        if available.len() != 4 { return Err("fewer than four valid shares".into()); }
        let mut matrix = [[0u8; 8]; 4];
        for (r, &index) in available.iter().enumerate() {
            matrix[r][..4].copy_from_slice(&row(index)); matrix[r][4 + r] = 1;
        }
        let inv = inverse(matrix)?;
        for j in 0..BLOCK_SIZE {
            for i in 0..4 {
                let mut value = 0;
                for r in 0..4 { value ^= mul(inv[i][r], shares[available[r]].as_ref().unwrap()[j]); }
                data[i][j] = value;
            }
        }
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_one_and_two_share_erasure_patterns() {
        for len in [600_123, DATA_SHARDS * BLOCK_SIZE] {
            let input: Vec<u8> = (0..len).map(|i| (i * 17 % 251) as u8).collect();
            let stripe = encode_stripe(&input).unwrap();
            let mut single_losses = 0;
            for a in 0..6 {
                let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                shares[a] = None;
                let recovered = reconstruct(shares, stripe.original_len, &stripe.shard_hashes).unwrap();
                assert_eq!(recovered, input, "single missing {a}, length {len}");
                assert_eq!(encode_stripe(&recovered).unwrap().shard_hashes, stripe.shard_hashes);
                single_losses += 1;
            }
            let (mut two_data, mut data_and_parity, mut two_parity) = (0, 0, 0);
            for a in 0..6 {
                for b in a + 1..6 {
                    let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                    shares[a] = None; shares[b] = None;
                    let recovered = reconstruct(shares, stripe.original_len, &stripe.shard_hashes).unwrap();
                    assert_eq!(recovered, input, "missing {a}, {b}, length {len}");
                    // reconstruct() returns original bytes; re-encoding restores lost parity too.
                    assert_eq!(encode_stripe(&recovered).unwrap().shard_hashes, stripe.shard_hashes);
                    match (a < DATA_SHARDS, b < DATA_SHARDS) {
                        (true, true) => two_data += 1,
                        (true, false) => data_and_parity += 1,
                        (false, false) => two_parity += 1,
                        (false, true) => unreachable!(),
                    }
                }
            }
            assert_eq!((single_losses, two_data, data_and_parity, two_parity), (6, 6, 8, 1));
            println!("custom_4plus2: len={len} single=6 two_data=6 data_plus_parity=8 two_parity=1 exact=true parity_regenerated=true");
        }
    }
    #[test]
    fn detects_corruption_and_repairs() {
        let stripe = encode_stripe(b"hello").unwrap();
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[0].as_mut().unwrap()[0] ^= 1;
        assert_eq!(reconstruct(shares, stripe.original_len, &stripe.shard_hashes).unwrap(), b"hello");
    }
}
