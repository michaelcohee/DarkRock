//! Measure actual RedTail-X share bytes for one prepacked byte stream.
use darkrock_storage::storage::{encode_stripe, reconstruct, STRIPE_SIZE};
use std::{env, fs};
fn main() {
    let path=env::args().nth(1).expect("packed stream path");
    let bytes=fs::read(path).expect("read packed stream");
    let mut coded=0usize;
    let mut stripes=0usize;
    for part in bytes.chunks(STRIPE_SIZE) {
        let stripe=encode_stripe(part).expect("encode");
        coded += stripe.shards.iter().map(Vec::len).sum::<usize>();
        for first in 0..6 {
            for second in first..6 {
                let mut shares:Vec<_>=stripe.shards.iter().cloned().map(Some).collect();
                shares[first]=None;
                if second != first { shares[second]=None; }
                let recovered=reconstruct(shares,&stripe.manifest).expect("one/two share recovery");
                assert_eq!(recovered,part);
            }
        }
        stripes+=1;
    }
    println!("packed_bytes={},stripes={},actual_share_bytes={},all_21_loss_patterns_exact=true",bytes.len(),stripes,coded);
}
