// SPDX-License-Identifier: MIT
use cobble_plugin_trust::{
    Result, create_key, create_root, public_key_id, sign_package, verify_package,
};
use std::{num::NonZeroU64, path::Path};
fn main() {
    if let Err(error) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(run())
    {
        eprintln!("{error}");
        std::process::exit(1)
    }
}
async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str){
 Some("key-create") if args.len()==2=>{let path=Path::new(&args[1]);create_key(path)?;println!("{}",serde_json::json!({"path":path,"public_key_id":public_key_id(path).await?}));},
 Some("root-create") if args.len()==6=>{create_root(Path::new(&args[1]),Path::new(&args[2]),Path::new(&args[3]),Path::new(&args[4]),args[5].parse()?).await?;println!("{}",serde_json::json!({"root":args[4]}));},
 Some("sign-package") if args.len()==7=>{let verified=sign_package(Path::new(&args[1]),Path::new(&args[2]),Path::new(&args[3]),Path::new(&args[4]),args[5].parse::<NonZeroU64>()?,args[6].parse()?).await?;println!("{}",serde_json::to_string_pretty(&verified)?);},
 Some("verify-package") if args.len()==3=>{println!("{}",serde_json::to_string_pretty(&verify_package(Path::new(&args[2]),&std::fs::read(&args[1])?).await?)?);},
 _=>return Err("Usage: key-create KEY | root-create ROOT_KEY PUBLISHER_KEY FEED_KEY NEW_ROOT_JSON EXPIRES | sign-package ROOT_JSON PUBLISHER_KEY FEED_KEY PACKAGE VERSION EXPIRES | verify-package ROOT_JSON PACKAGE".into())}
    Ok(())
}
