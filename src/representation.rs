//! Exact byte representation choice, independent of weighted network paths.
use sha2::{Digest, Sha256};

pub const CHUNK_SIZE: usize = 256 * 1024;
pub const METADATA_BYTES_PER_CHUNK: usize = 41; // tag + original len + payload len + SHA-256

pub const DEFAULT_PERIOD_LAGS: &[usize] = &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 6144, 7168, 7680];
pub const MIN_PERIOD_PAIRS: usize = 512;

#[derive(Clone, Debug)]
pub struct EntropyGateConfig {
    pub threshold_bits_per_byte: f64,
    pub sample_bytes: usize,
    pub period_lags: Vec<usize>,
    pub min_match_fraction: f64,
}

impl Default for EntropyGateConfig {
    fn default() -> Self {
        Self { threshold_bits_per_byte: 6.4, sample_bytes: 8192,
            period_lags: DEFAULT_PERIOD_LAGS.to_vec(), min_match_fraction: 0.05 }
    }
}

impl EntropyGateConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.threshold_bits_per_byte.is_finite() || self.threshold_bits_per_byte < 0.0
            || self.threshold_bits_per_byte > 8.0 || self.sample_bytes < 2
            || !self.min_match_fraction.is_finite() || self.min_match_fraction <= 0.0
            || self.min_match_fraction > 1.0 || self.period_lags.is_empty()
            || self.period_lags.iter().any(|&lag| lag == 0) {
            return Err("invalid entropy gate threshold, sample, lag set, or match fraction".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding { Raw, Zstd }

#[derive(Clone, Debug)]
pub struct Choice {
    pub encoding: Encoding,
    pub payload: Vec<u8>,
    pub original_len: usize,
    pub original_hash: [u8; 32],
    /// None means zstd was skipped, so no rejected encoded payload exists.
    pub rejected_encoding: Option<Encoding>,
    pub rejected_payload_len: Option<usize>,
    pub rejected_payload_hash: Option<[u8; 32]>,
    pub first_sample_entropy: f64,
    pub skipped_zstd: bool,
    pub matching_period_lag: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct GateDecision {
    pub try_zstd: bool,
    pub sample_entropy: f64,
    pub matching_period_lag: Option<usize>,
}

fn hash(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }

pub fn entropy_bits_per_byte(sample: &[u8]) -> f64 {
    if sample.is_empty() { return 0.0; }
    let mut counts = [0usize; 256];
    for &byte in sample { counts[byte as usize] += 1; }
    let size = sample.len() as f64;
    // Clamp the logarithm's input before evaluating it. Keep the original
    // probability as the weight so absent symbols contribute exactly zero.
    const MIN_LOG_PROBABILITY: f64 = 1e-12;
    counts.into_iter().map(|n| {
        let p = n as f64 / size;
        -p * p.clamp(MIN_LOG_PROBABILITY, 1.0).log2()
    }).sum()
}

/// Return the first lag with a match fraction above the configured cutoff.
pub fn periodic_lag(sample: &[u8], lags: &[usize], min_match_fraction: f64) -> Option<usize> {
    lags.iter().copied().filter(|&lag| lag < sample.len() && sample.len() - lag >= MIN_PERIOD_PAIRS).find(|&lag| {
        let pairs = sample.len() - lag;
        let matches = (0..pairs).filter(|&i| sample[i] == sample[i + lag]).count();
        matches as f64 / pairs as f64 >= min_match_fraction
    })
}

/// Inspect only the initial sample. This is a selection hint, not proof that
/// the full chunk is incompressible.
pub fn inspect_gate(chunk: &[u8], config: &EntropyGateConfig) -> Result<GateDecision, String> {
    inspect_gate_at(chunk, config, 0)
}

/// Inspect one contiguous sample beginning at `offset`. The production
/// selector uses offset zero; this variant supports controlled corpus tests.
pub fn inspect_gate_at(chunk: &[u8], config: &EntropyGateConfig, offset: usize) -> Result<GateDecision, String> {
    config.validate()?;
    let size = config.sample_bytes.min(chunk.len());
    if offset > chunk.len() - size { return Err("entropy sample offset exceeds chunk".into()); }
    let first = &chunk[offset..offset + size];
    let first_h = entropy_bits_per_byte(first);
    if first_h < config.threshold_bits_per_byte {
        return Ok(GateDecision { try_zstd: true, sample_entropy: first_h, matching_period_lag: None });
    }
    let lag = periodic_lag(first, &config.period_lags, config.min_match_fraction);
    Ok(GateDecision { try_zstd: lag.is_some(), sample_entropy: first_h, matching_period_lag: lag })
}

pub fn decode(encoding: Encoding, payload: &[u8]) -> Result<Vec<u8>, String> {
    match encoding {
        Encoding::Raw => Ok(payload.to_vec()),
        Encoding::Zstd => zstd::stream::decode_all(payload).map_err(|e| e.to_string()),
    }
}

/// Test both variants, verify exact decode equality, then choose lower bytes.
pub fn choose(chunk: &[u8]) -> Result<Choice, String> { choose_with_config(chunk, &EntropyGateConfig::default()) }

pub fn choose_with_config(chunk: &[u8], config: &EntropyGateConfig) -> Result<Choice, String> {
    let decision = inspect_gate(chunk, config)?;
    if !decision.try_zstd {
        return Ok(Choice {
            encoding: Encoding::Raw, payload: chunk.to_vec(), original_len: chunk.len(), original_hash: hash(chunk),
            rejected_encoding: None, rejected_payload_len: None, rejected_payload_hash: None,
            first_sample_entropy: decision.sample_entropy, skipped_zstd: true,
            matching_period_lag: decision.matching_period_lag,
        });
    }
    let compressed = zstd::stream::encode_all(chunk, 3).map_err(|e| e.to_string())?;
    if decode(Encoding::Zstd, &compressed)? != chunk { return Err("zstd verification failed".into()); }
    let (encoding, payload, rejected_encoding, rejected_payload) = if compressed.len() < chunk.len() {
        (Encoding::Zstd, compressed, Encoding::Raw, chunk.to_vec())
    } else {
        (Encoding::Raw, chunk.to_vec(), Encoding::Zstd, compressed)
    };
    Ok(Choice {
        encoding, payload, original_len: chunk.len(), original_hash: hash(chunk),
        rejected_encoding: Some(rejected_encoding), rejected_payload_len: Some(rejected_payload.len()), rejected_payload_hash: Some(hash(&rejected_payload)),
        first_sample_entropy: decision.sample_entropy, skipped_zstd: false,
        matching_period_lag: decision.matching_period_lag,
    })
}

pub fn restore(choices: &[Choice], packed: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new(); let mut offset: usize = 0;
    for choice in choices {
        let end = offset.checked_add(choice.payload.len()).ok_or("payload offset overflow")?;
        let segment = packed.get(offset..end).ok_or("missing packed segment")?;
        let decoded = decode(choice.encoding, segment)?;
        if decoded.len() != choice.original_len || hash(&decoded) != choice.original_hash {
            return Err("restored chunk differs".into());
        }
        output.extend_from_slice(&decoded);
        offset = end;
    }
    if offset != packed.len() { return Err("trailing packed bytes".into()); }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_round_trip_and_corruption_detection() {
        let input = [vec![0u8; CHUNK_SIZE], vec![7u8; 79]].concat();
        let choices: Vec<_> = input.chunks(CHUNK_SIZE).map(|c| choose(c).unwrap()).collect();
        let packed: Vec<_> = choices.iter().flat_map(|c| c.payload.iter().copied()).collect();
        assert_eq!(restore(&choices, &packed).unwrap(), input);
        let mut damaged = packed.clone(); damaged[0] ^= 1;
        assert!(restore(&choices, &damaged).is_err());
    }
    #[test]
    fn gate_skips_random_and_keeps_periodic_uniform_data() {
        let mut x = 0x1234_5678_9abc_def0_u64;
        let random: Vec<u8> = (0..CHUNK_SIZE).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect();
        assert!(choose(&random).unwrap().skipped_zstd);
        let periodic: Vec<u8> = (0..CHUNK_SIZE).map(|i| (i % 256) as u8).collect();
        let choice = choose(&periodic).unwrap();
        assert!(!choice.skipped_zstd);
        assert_eq!(choice.encoding, Encoding::Zstd);
    }
    #[test]
    fn short_and_four_kib_cycles_survive_high_entropy_gate() {
        for period in [2usize, 4, 256, 4096, 6144, 7168, 7680] {
            let motif: Vec<u8> = if period >= 4096 {
                let mut x = 0xabcdef0123456789_u64;
                (0..period).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect()
            } else { (0..period).map(|i| (i * 83) as u8).collect() };
            let data: Vec<u8> = (0..CHUNK_SIZE).map(|i| motif[i % period]).collect();
            let config = EntropyGateConfig::default();
            let choice = choose_with_config(&data, &config).unwrap();
            assert!(!choice.skipped_zstd, "period {period}");
            assert_eq!(choice.encoding, Encoding::Zstd);
            assert_eq!(periodic_lag(&data[..8192], &config.period_lags, config.min_match_fraction), Some(period));
        }
    }
    #[test]
    fn four_kib_period_needs_more_than_four_kib_sample() {
        let mut x = 0xabcdef0123456789_u64;
        let motif: Vec<u8> = (0..4096).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect();
        let data: Vec<u8> = (0..CHUNK_SIZE).map(|i| motif[i % 4096]).collect();
        let config = EntropyGateConfig { sample_bytes: 4096, ..Default::default() };
        assert_eq!(periodic_lag(&data[..4096], &config.period_lags, config.min_match_fraction), None);
        assert!(choose_with_config(&data, &config).unwrap().skipped_zstd);
    }
    #[test]
    fn single_sample_cannot_see_compressible_tail() {
        let mut x = 0x1234_5678_9abc_def0_u64;
        let mut data: Vec<u8> = (0..8192).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect();
        data.resize(CHUNK_SIZE, 0);
        let choice = choose(&data).unwrap();
        assert!(choice.skipped_zstd); // Deliberate limitation of the requested one-sample gate.
        assert!(zstd::stream::encode_all(data.as_slice(), 3).unwrap().len() < data.len() / 2);
    }
    #[test]
    fn sample_length_or_unlisted_lag_limits_detection() {
        let mut x = 0xabcdef0123456789_u64;
        let motif: Vec<u8> = (0..6144).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect();
        let data: Vec<u8> = (0..CHUNK_SIZE).map(|i| motif[i % motif.len()]).collect();
        let config = EntropyGateConfig { period_lags: vec![1, 2, 4, 8, 4096], ..Default::default() };
        assert!(inspect_gate(&data, &EntropyGateConfig::default()).unwrap().try_zstd);
        assert!(!inspect_gate(&data, &config).unwrap().try_zstd);
    }
    #[test]
    fn offset_inspection_uses_one_contiguous_sample() {
        let mut x = 0x1234_5678_9abc_def0_u64;
        let mut data: Vec<u8> = (0..8192).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect();
        data.resize(CHUNK_SIZE, 0);
        let config = EntropyGateConfig::default();
        assert!(!inspect_gate(&data, &config).unwrap().try_zstd);
        assert!(inspect_gate_at(&data, &config, (data.len() - config.sample_bytes) / 2).unwrap().try_zstd);
        assert!(inspect_gate_at(&data, &config, data.len()).is_err());
    }
}
