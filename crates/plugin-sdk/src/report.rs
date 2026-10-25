// SPDX-License-Identifier: MIT
use cobble_plugin_api::{MAX_FILE_BYTES, MRPACK_PAGE_PROTOCOL, PluginError, PluginManifest};
use std::{
    error::Error,
    fs,
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

fn media_type(manifest: &PluginManifest, input: &Path) -> Result<&'static str, PluginError> {
    if manifest.capabilities != ["report"] {
        return Err(PluginError::new(
            "PLUGIN_UNSUPPORTED_API",
            "Report test requires a report package",
        ));
    }
    let extension = input
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !manifest
        .file_extensions
        .iter()
        .any(|allowed| allowed == extension)
    {
        return Err(PluginError::new(
            "PLUGIN_UNSUPPORTED_INPUT",
            "Input extension is not supported by this report package",
        ));
    }
    match extension {
        "log" | "txt" => Ok("text/plain"),
        "json" | "mcmeta" => Ok("application/json"),
        "zip" if manifest.report_protocols == [cobble_plugin_api::PACK_METADATA_PROTOCOL] => {
            Ok("application/json")
        }
        "toml" => Ok("application/toml"),
        "properties" => Ok("text/x-java-properties"),
        "yaml" | "yml" => Ok("application/yaml"),
        "mrpack" => Ok("application/vnd.cobble.mrpack-inspection+json"),
        _ => Err(PluginError::new(
            "PLUGIN_UNSUPPORTED_INPUT",
            "Unsupported report input extension",
        )),
    }
}

fn prepare_input(
    manifest: &PluginManifest,
    input: &Path,
    baseline: Option<&Path>,
) -> Result<(Vec<u8>, &'static str), Box<dyn Error>> {
    let media_type = media_type(manifest, input)?;
    if baseline.is_some() && media_type != "application/vnd.cobble.mrpack-inspection+json" {
        return Err("--baseline requires .mrpack inputs".into());
    }
    let mut data = Vec::new();
    if input.extension().and_then(|value| value.to_str()) == Some("zip") {
        data = cobble_content::read_pack_metadata_zip(fs::File::open(input)?)
            .map_err(|error| error as Box<dyn Error>)?;
    } else if media_type == "application/vnd.cobble.mrpack-inspection+json" {
        let snapshot = |path: &Path| -> Result<_, Box<dyn Error>> {
            if self::media_type(manifest, path)? != media_type {
                return Err("--baseline requires .mrpack inputs".into());
            }
            let file = fs::File::open(path)?;
            if !file.metadata()?.is_file() {
                return Err("Report input must be a regular file".into());
            }
            cobble_content::inspect_mrpack_archive(file).map_err(|error| error as Box<dyn Error>)
        };
        let current = snapshot(input)?;
        let baseline = baseline.map(snapshot).transpose()?;
        data = serde_json::to_vec(&serde_json::json!({"current": current, "baseline": baseline}))?;
    } else {
        let file = fs::File::open(input)?;
        if !file.metadata()?.is_file() {
            return Err("Report input must be a regular file".into());
        }
        file.take(MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut data)?;
    }
    if data.len() > MAX_FILE_BYTES
        && !(media_type == "application/vnd.cobble.mrpack-inspection+json"
            && manifest.report_protocols == [MRPACK_PAGE_PROTOCOL])
    {
        return Err(PluginError::new(
            "PLUGIN_INPUT_TOO_LARGE",
            "Report input exceeds the size limit",
        )
        .into());
    }
    Ok((data, media_type))
}

pub fn test(
    package: &Path,
    host: &Path,
    input: &Path,
    options: &[String],
) -> Result<(), Box<dyn Error>> {
    let options = TestOptions::parse(options)?;
    let (manifest, component) = cobble_plugin_host::read_package(package)?;
    let context = if manifest.api_version == cobble_plugin_api::REPORT_CONTEXT_API_VERSION {
        Some(cobble_plugin_host::ReportHostContext::new(
            &manifest,
            &options.locale,
            options.settings.clone(),
        )?)
    } else {
        if options.locale != "en" || !options.settings.is_empty() {
            return Err("Context options require a report API 0.2.0 package".into());
        }
        None
    };
    let configured_page_size = options
        .settings
        .get("page_size")
        .cloned()
        .or_else(|| {
            manifest
                .contributions
                .as_ref()?
                .setting("page_size")
                .map(|field| field.default)
        })
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| (1..=100).contains(value))
        .unwrap_or(50);
    let (mut data, media_type) = prepare_input(&manifest, input, options.baseline.as_deref())?;
    let mut inspection = None;
    if manifest.report_protocols == [MRPACK_PAGE_PROTOCOL]
        && input
            .extension()
            .is_some_and(|extension| extension == "mrpack")
    {
        use cobble_modpack_inspector::{
            InspectionDetailQuery, InspectionEnvelope, InspectionQuery, InspectionRequest,
            PreparedInspection,
        };
        let request: InspectionRequest = serde_json::from_slice(&data)?;
        let prepared = PreparedInspection::new(request)
            .map_err(|error| format!("Invalid pack metadata: {error:?}"))?;
        let envelope = if let Some(id) = &options.detail {
            let query: InspectionDetailQuery = serde_json::from_value(
                serde_json::json!({"row_id":id,"field":options.field,"offset":options.offset,"limit":options.limit.unwrap_or(1024)}),
            )?;
            InspectionEnvelope::detail(
                prepared
                    .detail(&query)
                    .map_err(|error| format!("Invalid detail query: {error:?}"))?,
            )
        } else {
            let query: InspectionQuery = serde_json::from_value(
                serde_json::json!({"section":options.section,"search":options.search,"offset":options.offset,"limit":options.limit.unwrap_or(configured_page_size)}),
            )?;
            InspectionEnvelope::page(
                prepared
                    .query(&query)
                    .map_err(|error| format!("Invalid page query: {error:?}"))?,
            )
        };
        data = serde_json::to_vec(&envelope)?;
        inspection = Some(serde_json::to_value(envelope)?);
    } else if options.query_requested {
        return Err(
            "Page queries require a .mrpack input and a package opting into cobble.mrpack-page@1"
                .into(),
        );
    }
    if data.len() > MAX_FILE_BYTES {
        return Err("Selected report page exceeds the input size limit".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let report = if let Some(context) = context {
        cobble_plugin_host::run_report_with_context(
            host, &component, &data, media_type, cancel, &context,
        )?
    } else {
        cobble_plugin_host::run_report(host, &component, &data, media_type, cancel)?
    };
    if options.json {
        println!(
            "{}",
            serde_json::json!({"report_version":1,"plugin_id":manifest.id,"api_version":manifest.api_version,"media_type":media_type,"inspection":inspection,"report":report})
        );
    } else {
        println!(
            "{} findings; {} tables",
            report.findings.len(),
            report.tables.len()
        );
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    Ok(())
}

struct TestOptions {
    json: bool,
    baseline: Option<std::path::PathBuf>,
    section: String,
    search: String,
    offset: u64,
    limit: Option<u32>,
    detail: Option<String>,
    field: String,
    query_requested: bool,
    locale: String,
    settings: std::collections::BTreeMap<String, serde_json::Value>,
}
impl TestOptions {
    fn parse(args: &[String]) -> Result<Self, Box<dyn Error>> {
        let mut result = Self {
            json: false,
            baseline: None,
            section: "overview".into(),
            search: String::new(),
            offset: 0,
            limit: None,
            detail: None,
            field: "path".into(),
            query_requested: false,
            locale: "en".into(),
            settings: std::collections::BTreeMap::new(),
        };
        let mut args = args.iter();
        let mut seen = std::collections::HashSet::new();
        while let Some(flag) = args.next() {
            if flag == "--setting" {
                let assignment = args.next().ok_or("--setting requires KEY=JSON")?;
                let (key, value) = assignment
                    .split_once('=')
                    .ok_or("--setting requires KEY=JSON")?;
                if key.is_empty()
                    || result
                        .settings
                        .insert(key.into(), serde_json::from_str(value)?)
                        .is_some()
                {
                    return Err("Duplicate or empty setting ID".into());
                }
                continue;
            }
            if !seen.insert(flag.as_str()) {
                return Err(format!("Duplicate option: {flag}").into());
            }
            if flag == "--json" {
                result.json = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{flag} requires a value"))?;
            if !matches!(flag.as_str(), "--baseline" | "--locale") {
                result.query_requested = true;
            }
            match flag.as_str() {
                "--baseline" => result.baseline = Some(value.into()),
                "--section" => result.section = value.clone(),
                "--search" => result.search = value.clone(),
                "--offset" => result.offset = value.parse()?,
                "--limit" => result.limit = Some(value.parse()?),
                "--detail" => result.detail = Some(value.clone()),
                "--field" => result.field = value.clone(),
                "--locale" => result.locale = value.clone(),
                _ => return Err(format!("Unknown option: {flag}").into()),
            }
        }
        if seen.contains("--field") && result.detail.is_none() {
            return Err("--field requires --detail ROW_ID".into());
        }
        if result.detail.is_some() && (seen.contains("--section") || seen.contains("--search")) {
            return Err("--detail cannot be combined with --section or --search".into());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_options_preserve_queries_and_reject_ambiguous_requests() {
        let parse = |args: &[&str]| {
            TestOptions::parse(&args.iter().map(|value| (*value).into()).collect::<Vec<_>>())
        };
        let options = parse(&[
            "--section",
            "changes",
            "--search",
            "配置",
            "--offset",
            "100",
            "--limit",
            "50",
            "--json",
        ])
        .unwrap();
        assert_eq!(
            (
                options.section.as_str(),
                options.search.as_str(),
                options.offset,
                options.limit
            ),
            ("changes", "配置", 100, Some(50))
        );
        let context = parse(&[
            "--locale",
            "zh-CN",
            "--setting",
            "show_dependencies=false",
            "--setting",
            "page_size=20",
        ])
        .unwrap();
        assert_eq!(context.locale, "zh-CN");
        assert_eq!(
            context.settings["show_dependencies"],
            serde_json::json!(false)
        );
        assert_eq!(context.settings["page_size"], serde_json::json!(20));
        assert!(parse(&["--setting", "x=true", "--setting", "x=false"]).is_err());
        assert!(!context.query_requested);
        assert!(options.json);
        assert!(parse(&["--offset", "-1"]).is_err());
        assert!(parse(&["--limit", "2", "--limit", "3"]).is_err());
        assert!(parse(&["--field", "after"]).is_err());
        assert!(parse(&["--detail", "row-1", "--section", "changes"]).is_err());
        let detail = parse(&["--detail", "row-1", "--field", "after", "--offset", "512"]).unwrap();
        assert_eq!(
            (
                detail.detail.as_deref(),
                detail.field.as_str(),
                detail.offset
            ),
            (Some("row-1"), "after", 512)
        );
    }

    #[test]
    fn binds_media_type_to_declared_extensions_and_capability() {
        let mut manifest = PluginManifest {
            frontend: None,
            permissions: None,
            description: None,
            tags: Vec::new(),
            icon: None,
            id: "report".into(),
            name: "Report".into(),
            version: "0.1.0".into(),
            api_version: cobble_plugin_api::REPORT_API_VERSION.into(),
            publisher: "Original fixture".into(),
            capabilities: vec!["report".into()],
            tool_form: None,
            composer_permissions: Default::default(),
            interfaces: Default::default(),
            file_extensions: vec!["log".into(), "txt".into(), "json".into(), "mrpack".into()],
            report_protocols: vec![],
            contributions: None,
            provider_origins: vec![],
            component_sha256: "a".repeat(64),
        };
        assert_eq!(
            media_type(&manifest, Path::new("input.log")).unwrap(),
            "text/plain"
        );
        assert_eq!(
            media_type(&manifest, Path::new("input.txt")).unwrap(),
            "text/plain"
        );
        assert_eq!(
            media_type(&manifest, Path::new("input.json")).unwrap(),
            "application/json"
        );
        assert_eq!(
            media_type(&manifest, Path::new("input.mrpack")).unwrap(),
            "application/vnd.cobble.mrpack-inspection+json"
        );
        assert!(
            prepare_input(
                &manifest,
                Path::new("input.json"),
                Some(Path::new("baseline.mrpack"))
            )
            .unwrap_err()
            .to_string()
            .contains("--baseline requires")
        );
        manifest.file_extensions = vec!["log".into()];
        assert!(media_type(&manifest, Path::new("input.json")).is_err());
        manifest.capabilities = vec!["viewer".into()];
        assert_eq!(
            media_type(&manifest, Path::new("input.log"))
                .unwrap_err()
                .code,
            "PLUGIN_UNSUPPORTED_API"
        );
    }
}
