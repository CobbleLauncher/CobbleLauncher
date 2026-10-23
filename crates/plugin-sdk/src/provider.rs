// SPDX-License-Identifier: MIT
//! Actual Component planning/normalization using explicit offline response files.
use cobble_plugin_host::provider::{GetResponse, ProviderRequest, ProviderSession};
use std::{
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

fn bounded_file(path: &Path, maximum: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err("Provider fixture must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err("Provider fixture exceeds its byte budget".into());
    }
    Ok(bytes)
}
fn options(args: &[String]) -> Result<(Vec<PathBuf>, bool), Box<dyn Error>> {
    let mut json = false;
    let mut paths = Vec::new();
    for arg in args {
        if arg == "--json" && !json {
            json = true;
        } else if arg.starts_with("--") {
            return Err("Expected one or two response paths and optional --json".into());
        } else {
            paths.push(PathBuf::from(arg));
        }
    }
    if paths.is_empty() || paths.len() > 2 {
        return Err("Supply one or two ordered HTTP 200 response fixture paths".into());
    }
    Ok((paths, json))
}
pub fn test(
    package: &Path,
    host: &Path,
    request_path: &Path,
    args: &[String],
) -> Result<(), Box<dyn Error>> {
    let (response_paths, json) = options(args)?;
    let (manifest, component) = cobble_plugin_host::read_package(package)?;
    if manifest.api_version != cobble_plugin_api::PROVIDER_API_VERSION
        || manifest.capabilities != ["blueprint-provider"]
    {
        return Err("Provider test requires a blueprint-provider package".into());
    }
    let request: ProviderRequest = serde_json::from_slice(&bounded_file(request_path, 65536)?)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let mut session = ProviderSession::open(host, &component, cancel.clone())?;
    let plan = session.plan(&request, &manifest.provider_origins, cancel.clone())?;
    if plan.len() != response_paths.len() {
        return Err("Fixture response count does not match the actual provider GET plan".into());
    }
    let responses = plan
        .iter()
        .zip(response_paths)
        .map(|(get, path)| {
            Ok(GetResponse {
                status: 200,
                body: bounded_file(&path, get.maximum_bytes as usize)?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let output = session.normalize(&request, &responses, &manifest.provider_origins, cancel)?;
    session.close();
    if !session.has_exited()? {
        return Err("Provider worker did not exit".into());
    }
    if json {
        println!(
            "{}",
            serde_json::json!({"report_version":1,"plugin_id":manifest.id,"api_version":manifest.api_version,"mode":"offline-fixtures","planned_gets":plan,"output":output})
        );
    } else {
        println!(
            "{} approved GETs; {} resources; detail {}; download {} (offline fixtures)",
            plan.len(),
            output.items.len(),
            output.detail.is_some(),
            output.download.is_some()
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn response_arguments_are_finite_and_never_trigger_network() {
        let args = |items: &[&str]| {
            items
                .iter()
                .map(|item| (*item).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            super::options(&args(&["response.json", "--json"]))
                .unwrap()
                .0
                .len(),
            1
        );
        assert!(super::options(&args(&[])).is_err());
        assert!(super::options(&args(&["a", "b", "c"])).is_err());
        assert!(super::options(&args(&["a", "--json", "--json"])).is_err());
        assert!(super::options(&args(&["a", "--fetch"])).is_err());
    }
}
