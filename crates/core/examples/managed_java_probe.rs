// SPDX-License-Identifier: MIT
//! Authorized isolated Java acquisition; never starts Minecraft.
use cobble_core::{OperationControl, install_managed_java_at_controlled};
use std::sync::Arc;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/managed-java-probes/java17");
    std::fs::create_dir_all(&root)?;
    let control = OperationControl::new(Arc::new(|progress| {
        if progress.detail.is_none() {
            println!(
                "{} {}/{}",
                progress.phase, progress.completed, progress.total
            );
        }
    }));
    let mut result = install_managed_java_at_controlled(
        17,
        &root.join("runtimes"),
        &root.join("cache"),
        control.clone(),
    )
    .await;
    if result.as_ref().err().is_some_and(|error| matches!(error.downcast_ref::<cobble_net::ManagedDownloadError>(), Some(cobble_net::ManagedDownloadError::Download(error)) if matches!(error.as_ref(),cobble_net::DownloadError::Deadline))) {
        println!("Observed bounded download deadline; retrying once through the same managed cache");
        result = install_managed_java_at_controlled(17,&root.join("runtimes"),&root.join("cache"),control.clone()).await;
    }
    let runtime = result?;
    assert_eq!(runtime.major, 17);
    println!(
        "Verified Java {} {} {}",
        runtime.major, runtime.version, runtime.arch
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(
        runtime
            .executable
            .parent()
            .ok_or("missing bin")?
            .parent()
            .ok_or("missing runtime")?
            .join(".cobble-runtime.json"),
    )?)?;
    println!("Provenance {}", serde_json::to_string(&marker)?);
    let again = install_managed_java_at_controlled(
        17,
        &root.join("runtimes"),
        &root.join("cache"),
        control,
    )
    .await?;
    assert_eq!(runtime.executable, again.executable);
    println!("Cached runtime reused");
    let output = tokio::process::Command::new(&runtime.executable)
        .arg("-version")
        .output()
        .await?;
    assert!(output.status.success());
    println!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(())
}
