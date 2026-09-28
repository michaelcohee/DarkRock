//! Pack length-framed objects using the production representation selector.
use darkrock_storage::representation::{choose, decode};
use sha2::{Digest, Sha256};
use std::{env, fs::File, io::{BufReader, BufWriter, Read, Write}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args.next().ok_or("length-framed input path required")?;
    let output = args.next().ok_or("packed output path required")?;
    let index = args.next();
    if args.next().is_some() { return Err("too many arguments".into()); }
    let mut reader = BufReader::new(File::open(input)?);
    let mut writer = BufWriter::new(File::create(output)?);
    let mut index_writer = match index { Some(path) => Some(BufWriter::new(File::create(path)?)), None => None };
    let mut objects = 0usize;
    let mut raw = 0usize;
    let mut selected = 0usize;
    let mut zstd = 0usize;
    let mut skipped = 0usize;
    loop {
        let mut header = [0u8; 8];
        let mut filled = 0;
        while filled < header.len() {
            let n = reader.read(&mut header[filled..])?;
            if n == 0 {
                if filled == 0 {
                    writer.flush()?;
                    if let Some(w) = index_writer.as_mut() { w.flush()?; }
                    println!("objects={objects},raw_bytes={raw},selected_bytes={selected},zstd_objects={zstd},skipped_zstd={skipped}");
                    return Ok(());
                }
                return Err("truncated length frame".into());
            }
            filled += n;
        }
        let len = usize::try_from(u64::from_le_bytes(header))?;
        if len == 0 || len > 64 * 1024 * 1024 { return Err("invalid object size".into()); }
        let mut object = vec![0u8; len];
        reader.read_exact(&mut object)?;
        let choice = choose(&object)?;
        let restored = decode(choice.encoding, &choice.payload)?;
        if restored != object || Sha256::digest(&restored).as_slice() != choice.original_hash {
            return Err("selected object failed byte/hash restoration".into());
        }
        writer.write_all(&choice.payload)?;
        if let Some(w) = index_writer.as_mut() {
            let kind = if choice.encoding == darkrock_storage::representation::Encoding::Zstd { "zstd" } else { "raw" };
            let hash: String = choice.original_hash.iter().map(|byte| format!("{byte:02x}")).collect();
            writeln!(w, "{len}\t{}\t{kind}\t{hash}", choice.payload.len())?;
        }
        objects += 1;
        raw += len;
        selected += choice.payload.len();
        if choice.encoding == darkrock_storage::representation::Encoding::Zstd { zstd += 1; }
        if choice.skipped_zstd { skipped += 1; }
    }
}
