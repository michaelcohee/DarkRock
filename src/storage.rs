//! Byte-level identity and Reed-Solomon erasure over GF(2^8).
use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};

pub const BLOCK_SIZE: usize = 256 * 1024;
pub const DATA_SHARDS: usize = 4;
pub const PARITY_SHARDS: usize = 2;
pub const STRIPE_SIZE: usize = DATA_SHARDS * BLOCK_SIZE;

#[derive(Clone, Debug)]
pub struct ChunkRef { pub hash: [u8; 32], pub length: usize }

/// Raw byte IDs are for one trust domain; shared dedup needs a privacy design.
pub fn chunk_refs(bytes: &[u8]) -> Vec<ChunkRef> {
    bytes.chunks(BLOCK_SIZE).map(|chunk| ChunkRef {
        hash: Sha256::digest(chunk).into(), length: chunk.len(),
    }).collect()
}

/// Original length determines final-share length and is checked against the
/// original hash after reconstruction. A remote manifest still needs authentication.
#[derive(Clone, Debug)]
pub struct StripeManifest {
    pub original_len: usize,
    pub share_len: usize,
    pub original_hash: [u8; 32],
    pub shard_hashes: Vec<[u8; 32]>,
}

impl StripeManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.original_len == 0 || self.original_len > STRIPE_SIZE
            || self.share_len != self.original_len.div_ceil(DATA_SHARDS)
            || self.share_len > BLOCK_SIZE
            || self.shard_hashes.len() != DATA_SHARDS + PARITY_SHARDS {
            return Err("invalid stripe manifest length or share count".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Stripe { pub shards: Vec<Vec<u8>>, pub manifest: StripeManifest }

fn codec() -> Result<ReedSolomon, String> {
    ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS).map_err(|e| e.to_string())
}

/// Full stripes use 256 KiB shares; partial stripes use ceil(input bytes / 4).
/// Caller handles encryption, manifest authentication, and persistence.
pub fn encode_stripe(bytes: &[u8]) -> Result<Stripe, String> {
    if bytes.is_empty() || bytes.len() > STRIPE_SIZE { return Err("stripe must contain 1 byte to 1 MiB".into()); }
    let share_len = bytes.len().div_ceil(DATA_SHARDS);
    let mut shards = vec![vec![0u8; share_len]; DATA_SHARDS + PARITY_SHARDS];
    for (dst, src) in shards[..DATA_SHARDS].iter_mut().zip(bytes.chunks(share_len)) {
        dst[..src.len()].copy_from_slice(src);
    }
    codec()?.encode(&mut shards).map_err(|e| e.to_string())?;
    let manifest = StripeManifest {
        original_len: bytes.len(), share_len,
        original_hash: Sha256::digest(bytes).into(),
        shard_hashes: shards.iter().map(|s| Sha256::digest(s).into()).collect(),
    };
    Ok(Stripe { shards, manifest })
}

/// Reconstruct and verify the exact original bytes using the stored manifest.
pub fn reconstruct(mut shares: Vec<Option<Vec<u8>>>, manifest: &StripeManifest) -> Result<Vec<u8>, String> {
    manifest.validate()?;
    if shares.len() != DATA_SHARDS + PARITY_SHARDS { return Err("invalid share count".into()); }
    for (share, hash) in shares.iter_mut().zip(&manifest.shard_hashes) {
        if let Some(data) = share {
            if data.len() != manifest.share_len || Sha256::digest(data.as_slice()).as_slice() != hash {
                *share = None;
            }
        }
    }
    codec()?.reconstruct_data(&mut shares).map_err(|e| e.to_string())?;
    let mut output = Vec::with_capacity(DATA_SHARDS * manifest.share_len);
    for (share, hash) in shares.into_iter().zip(&manifest.shard_hashes).take(DATA_SHARDS) {
        let data = share.ok_or("missing reconstructed data share")?;
        if data.len() != manifest.share_len || Sha256::digest(&data).as_slice() != hash {
            return Err("reconstructed share hash mismatch".into());
        }
        output.extend_from_slice(&data);
    }
    output.truncate(manifest.original_len);
    if Sha256::digest(&output).as_slice() != manifest.original_hash {
        return Err("reconstructed stripe length or content differs".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify_all_losses(bytes: &[u8]) {
        let stripe = encode_stripe(bytes).unwrap();
        let expected_share_len = bytes.len().div_ceil(DATA_SHARDS);
        assert_eq!(stripe.manifest.share_len, expected_share_len);
        assert!(stripe.shards.iter().all(|s| s.len() == expected_share_len));
        for first in 0..DATA_SHARDS + PARITY_SHARDS {
            let mut one: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
            one[first] = None;
            assert_eq!(reconstruct(one, &stripe.manifest).unwrap(), bytes);
            for second in first + 1..DATA_SHARDS + PARITY_SHARDS {
                let mut two: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                two[first] = None; two[second] = None;
                assert_eq!(reconstruct(two, &stripe.manifest).unwrap(), bytes, "missing {first}, {second}");
            }
        }
    }

    #[test]
    fn variable_final_share_boundaries_and_all_one_two_loss_patterns() {
        for len in [1, 2, 3, 4, 5, 10 * 1024, BLOCK_SIZE - 1, BLOCK_SIZE,
            BLOCK_SIZE + 1, STRIPE_SIZE - 1, STRIPE_SIZE] {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 41) as u8).collect();
            verify_all_losses(&bytes);
        }
    }

    #[test]
    fn mixed_full_and_partial_stripes_reconstruct_in_one_file() {
        let len = 2 * STRIPE_SIZE + 10 * 1024;
        let input: Vec<u8> = (0..len).map(|i| (i * 83) as u8).collect();
        let stripes: Vec<_> = input.chunks(STRIPE_SIZE).map(|part| encode_stripe(part).unwrap()).collect();
        assert_eq!(stripes.len(), 3);
        assert_eq!(stripes[0].manifest.share_len, BLOCK_SIZE);
        assert_eq!(stripes[1].manifest.share_len, BLOCK_SIZE);
        assert_eq!(stripes[2].manifest.share_len, 2560);
        let mut output = Vec::new();
        for stripe in &stripes {
            let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
            shares[0] = None; shares[4] = None;
            output.extend(reconstruct(shares, &stripe.manifest).unwrap());
        }
        assert_eq!(output, input);
    }

    #[test]
    fn damaged_or_missing_length_fails_loudly() {
        let stripe = encode_stripe(&vec![7_u8; 10 * 1024]).unwrap();
        let shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        let mut manifest = stripe.manifest.clone();
        manifest.original_len += 1;
        assert!(reconstruct(shares.clone(), &manifest).is_err());
        manifest = stripe.manifest.clone(); manifest.share_len = 0;
        assert!(reconstruct(shares.clone(), &manifest).is_err());
        manifest = stripe.manifest.clone(); manifest.original_len = 0;
        assert!(reconstruct(shares.clone(), &manifest).is_err());
        manifest = stripe.manifest.clone(); manifest.shard_hashes.pop();
        assert!(reconstruct(shares, &manifest).is_err());
    }

    #[test]
    fn rejects_three_missing_shares_and_detects_corruption() {
        let stripe = encode_stripe(b"hello").unwrap();
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[0] = None; shares[1] = None; shares[2] = None;
        assert!(reconstruct(shares, &stripe.manifest).is_err());
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        shares[4].as_mut().unwrap()[0] ^= 1; shares[0] = None;
        assert_eq!(reconstruct(shares, &stripe.manifest).unwrap(), b"hello");
    }
}
