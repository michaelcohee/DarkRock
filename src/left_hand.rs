//! Integer-expression canonicalization, separate from byte dedup and erasure.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftHandStore {
    target: i64,
    entries: Vec<Vec<i64>>,
    primary_index: Option<usize>,
    rejected: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsolationReceipt {
    pub kept_primary: Vec<i64>,
    pub kept_payload_bytes: usize,
    pub control_verified: bool,
    pub rejected_count: usize,
    pub dropped_count: usize,
    pub dropped_payload_bytes: usize,
    /// Exact discarded entries. The caller must persist this for an audit trail.
    pub dropped_entries: Vec<Vec<i64>>,
}

/// The source session's unsigned-magnitude estimate; excludes sign, lengths,
/// container headers, receipt, control record, allocator and file-system costs.
pub fn estimated_integer_bytes(value: i64) -> usize {
    let mut magnitude = value.unsigned_abs();
    let mut bytes = 1;
    while magnitude > 255 { magnitude >>= 8; bytes += 1; }
    bytes
}

pub fn estimated_entry_bytes(entry: &[i64]) -> usize {
    entry.iter().map(|&v| estimated_integer_bytes(v)).sum()
}

impl LeftHandStore {
    pub fn new(target: i64) -> Self {
        Self { target, entries: Vec::new(), primary_index: None, rejected: 0 }
    }

    pub fn ingest(&mut self, entries: impl IntoIterator<Item = Vec<i64>>) {
        for entry in entries {
            let valid = entry.iter().try_fold(0_i128, |sum, &v| sum.checked_add(v as i128))
                == Some(self.target as i128);
            if !valid { self.rejected += 1; continue; }
            let cost = estimated_entry_bytes(&entry);
            if self.primary_index.map(|i| cost < estimated_entry_bytes(&self.entries[i])).unwrap_or(true) {
                self.primary_index = Some(self.entries.len());
            }
            self.entries.push(entry);
        }
    }

    pub fn verified_count(&self) -> usize { self.entries.len() }
    pub fn estimated_payload_bytes(&self) -> usize {
        self.entries.iter().map(|e| estimated_entry_bytes(e)).sum()
    }

    /// Approximate Rust allocation footprint, excluding allocator metadata.
    pub fn allocated_bytes_estimate(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.entries.capacity() * std::mem::size_of::<Vec<i64>>()
            + self.entries.iter().map(|e| e.capacity() * std::mem::size_of::<i64>()).sum::<usize>()
    }

    pub fn isolate_primary(&mut self) -> Option<IsolationReceipt> {
        let primary_index = self.primary_index?;
        let mut dropped_entries = Vec::with_capacity(self.entries.len().saturating_sub(1));
        let mut kept = None;
        for (index, entry) in self.entries.drain(..).enumerate() {
            if index == primary_index { kept = Some(entry); } else { dropped_entries.push(entry); }
        }
        let kept_primary = kept.unwrap();
        let kept_payload_bytes = estimated_entry_bytes(&kept_primary);
        let dropped_payload_bytes = dropped_entries.iter().map(|e| estimated_entry_bytes(e)).sum();
        let dropped_count = dropped_entries.len();
        self.entries.push(kept_primary.clone());
        self.entries.shrink_to_fit();
        self.primary_index = Some(0);
        Some(IsolationReceipt {
            kept_primary, kept_payload_bytes, control_verified: true,
            rejected_count: self.rejected, dropped_count, dropped_payload_bytes, dropped_entries,
        })
    }
}

impl IsolationReceipt {
    /// The receipt still owns the discarded vectors until archived or dropped.
    pub fn allocated_bytes_estimate(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.kept_primary.capacity() * std::mem::size_of::<i64>()
            + self.dropped_entries.capacity() * std::mem::size_of::<Vec<i64>>()
            + self.dropped_entries.iter().map(|e| e.capacity() * std::mem::size_of::<i64>()).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_selects_and_receipts() {
        let mut store = LeftHandStore::new(2);
        store.ingest([vec![2], vec![1, 1], vec![10, -10, 2], vec![3]]);
        assert_eq!(store.verified_count(), 3);
        assert_eq!(store.estimated_payload_bytes(), 6);
        let receipt = store.isolate_primary().unwrap();
        assert_eq!(receipt.kept_primary, vec![2]);
        assert_eq!(receipt.dropped_count, 2);
        assert_eq!(receipt.dropped_payload_bytes, 5);
        assert_eq!(receipt.rejected_count, 1);
        assert_eq!(store.estimated_payload_bytes(), 1);
    }
    #[test]
    fn empty_and_negative() {
        let mut store = LeftHandStore::new(-2);
        assert!(store.isolate_primary().is_none());
        store.ingest([vec![-2], vec![i64::MIN, i64::MAX, -1]]);
        assert_eq!(store.verified_count(), 2);
        assert_eq!(store.isolate_primary().unwrap().kept_primary, vec![-2]);
    }
}
