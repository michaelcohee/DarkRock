//! Print the frozen A3 v2 candidate geometry keys for one ASCII DXF.
use darkrock_storage::dxf_adapter::geometry_keys;
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args_os().nth(1).ok_or("DXF path required")?;
    let raw = fs::read(path)?;
    for key in geometry_keys(&raw)? { println!("{key}"); }
    Ok(())
}
