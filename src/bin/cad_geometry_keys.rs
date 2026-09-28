use darkrock_storage::dxf_adapter::{geometry_keys,geometry_coverage};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut args=std::env::args().skip(1);let first=args.next().ok_or("DXF path required")?;
    let (coverage,path)=if first=="--coverage" {(true,args.next().ok_or("DXF path required")?)}else{(false,first)};
    let bytes=std::fs::read(path)?;
    if coverage {let (parsed,raw,entities)=geometry_coverage(&bytes)?;println!("{parsed}\t{raw}\t{entities}")}
    else {for key in geometry_keys(&bytes)?{println!("{key}")}}Ok(())
}
