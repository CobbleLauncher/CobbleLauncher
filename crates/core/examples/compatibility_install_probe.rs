// SPDX-License-Identifier: MIT
//! Opt-in isolated NeoForge installation using the production installer and cache.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let target = PathBuf::from("target/compatibility-probes/neoforge-1.21.4-install");
    let last = Arc::new(Mutex::new(String::new()));
    let saved = last.clone();
    let control = cobble_core::OperationControl::new(Arc::new(move |progress| {
        let mut phase = saved.lock().unwrap();
        if *phase != progress.phase {
            eprintln!("stage={}", progress.phase);
            *phase = progress.phase;
        }
    }));
    let operation = cobble_core::install_forge_controlled(
        "1.21.4",
        "21.4.157",
        cobble_core::ForgeKind::NeoForge,
        &target,
        cobble_minecraft::DownloadSource::Automatic,
        control.clone(),
    );
    tokio::pin!(operation);
    let result = tokio::select! { result = &mut operation => result, _ = tokio::time::sleep(Duration::from_secs(900)) => { control.cancel(); operation.await } };
    match result {
        Ok(outcome) => {
            println!(
                "installed={} java={} complete_marker={}",
                outcome.version,
                outcome.java_major,
                target.join(".cobble-forge.json").is_file()
            );
            Ok(())
        }
        Err(error) => {
            eprintln!("failed_stage={} error={error}", last.lock().unwrap());
            Err(error)
        }
    }
}
