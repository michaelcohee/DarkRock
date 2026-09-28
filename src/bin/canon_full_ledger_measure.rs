//! Encode a complete serialized ledger and its stripe-manifest catalog through 4+2.
use darkrock_storage::storage::{encode_stripe,reconstruct,StripeManifest,STRIPE_SIZE};
use sha2::{Digest,Sha256};
use std::{env,fs};

fn check_all(part:&[u8])->Result<(usize,StripeManifest),String>{
    let stripe=encode_stripe(part)?;
    for first in 0..6 {for second in first..6 {
        let mut shares:Vec<_>=stripe.shards.iter().cloned().map(Some).collect();shares[first]=None;
        if second!=first{shares[second]=None}
        if reconstruct(shares,&stripe.manifest)?!=part{return Err("share-loss reconstruction differed".into())}
    }}
    Ok((stripe.shards.iter().map(Vec::len).sum(),stripe.manifest))
}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let path=env::args().nth(1).ok_or("ledger path required")?;
    let bytes=fs::read(path)?;if bytes.is_empty(){return Err("empty ledger".into())}
    let mut data_share=0;let mut manifests=Vec::new();
    for part in bytes.chunks(STRIPE_SIZE){let (count,manifest)=check_all(part)?;data_share+=count;
        manifests.extend_from_slice(&(manifest.original_len as u64).to_le_bytes());
        manifests.extend_from_slice(&(manifest.share_len as u64).to_le_bytes());
        manifests.extend_from_slice(&manifest.original_hash);
        for h in &manifest.shard_hashes{manifests.extend_from_slice(h)}
    }
    let mut catalog=Vec::new();catalog.extend_from_slice(b"DRMC1");catalog.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    catalog.extend_from_slice(&(manifests.len()/240).to_le_bytes());catalog.extend_from_slice(&manifests);
    let mut manifest_share=0;let mut manifest_stripes=0;
    for part in catalog.chunks(STRIPE_SIZE){let (count,_)=check_all(part)?;manifest_share+=count;manifest_stripes+=1}
    // Finite bootstrap: six copies of the catalog-stripe manifests and
    // a 32-byte authenticated catalog digest. This is the non-recursive root.
    let root_hash=Sha256::digest(&catalog);assert_eq!(root_hash.len(),32);
    let bootstrap=6*(32+240*manifest_stripes);
    println!("ledger_bytes={},data_stripes={},data_share_bytes={},manifest_catalog_bytes={},manifest_stripes={},manifest_share_bytes={},bootstrap_copies=6,bootstrap_bytes={},total_physical_bytes={},all_21_loss_patterns_exact=true",bytes.len(),manifests.len()/240,data_share,catalog.len(),manifest_stripes,manifest_share,bootstrap,data_share+manifest_share+bootstrap);
    Ok(())
}
