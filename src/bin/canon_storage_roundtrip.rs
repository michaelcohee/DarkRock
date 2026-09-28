//! Reconstruct each 4+2 stripe under all 21 one/two-share losses and emit a recovered stream.
use darkrock_storage::storage::{encode_stripe, reconstruct, STRIPE_SIZE};
use sha2::{Digest, Sha256};
use std::{env, fs::File, io::{BufReader, BufWriter, Read, Write}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args.next().ok_or("packed input required")?;
    let output = args.next().ok_or("recovered output required")?;
    if args.next().is_some() { return Err("too many arguments".into()); }
    let mut reader = BufReader::new(File::open(input)?);
    let stream_output = output == "-";
    let mut writer: Box<dyn Write> = if stream_output {
        Box::new(std::io::stdout())
    } else {
        Box::new(BufWriter::new(File::create(output)?))
    };
    let mut stripes = 0usize;
    let mut bytes = 0usize;
    let mut share_bytes = 0usize;
    let mut input_hash = Sha256::new();
    let mut recovered_hash = Sha256::new();
    loop {
        let mut block = vec![0u8; STRIPE_SIZE];
        let mut used = 0usize;
        while used < block.len() {
            let count = reader.read(&mut block[used..])?;
            if count == 0 { break; }
            used += count;
        }
        if used == 0 { break; }
        block.truncate(used);
        input_hash.update(&block);
        let stripe = encode_stripe(&block)?;
        share_bytes += stripe.shards.iter().map(Vec::len).sum::<usize>();
        let mut emitted = false;
        for first in 0..6 {
            for second in first..6 {
                let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
                shares[first] = None;
                if second != first { shares[second] = None; }
                let rebuilt = reconstruct(shares, &stripe.manifest)?;
                if rebuilt != block { return Err(format!("stripe {stripes}, losses {first}/{second}: mismatch").into()); }
                if first == 0 && second == 1 {
                    writer.write_all(&rebuilt)?;
                    recovered_hash.update(&rebuilt);
                    emitted = true;
                }
            }
        }
        if !emitted { return Err("no two-loss reconstruction emitted".into()); }
        stripes += 1;
        bytes += used;
    }
    writer.flush()?;
    if input_hash.finalize().as_slice() != recovered_hash.finalize().as_slice() {
        return Err("recovered stream hash mismatch".into());
    }
    let summary = format!("stripes={stripes},packed_bytes={bytes},share_bytes={share_bytes},patterns_per_stripe=21,recovered_exact=true");
    if stream_output { eprintln!("{summary}"); } else { println!("{summary}"); }
    Ok(())
}
