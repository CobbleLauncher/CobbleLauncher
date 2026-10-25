// SPDX-License-Identifier: MIT
//! Bounded declaration graph, including nested Fabric JARs as part of their parent file.
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Seek},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
const ERROR: &str = "EXPERIMENT_DEPENDENCY_METADATA_UNAVAILABLE";
#[derive(Default)]
struct Declarations {
    ids: BTreeSet<String>,
    required: BTreeSet<String>,
}
fn id(value: &Value) -> Result<String, String> {
    let id = value.as_str().ok_or(ERROR)?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        return Err(ERROR.into());
    }
    Ok(id.into())
}
fn bytes<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
    remaining: &mut u64,
) -> Result<Vec<u8>, String> {
    let limit = limit.min(*remaining);
    let mut entry = archive.by_name(name).map_err(|_| ERROR)?;
    if entry.size() > limit || entry.size() > *remaining {
        return Err(ERROR.into());
    }
    let mut result = vec![];
    entry
        .by_ref()
        .take(limit + 1)
        .read_to_end(&mut result)
        .map_err(|_| ERROR)?;
    if result.len() as u64 > limit {
        return Err(ERROR.into());
    }
    *remaining = remaining.saturating_sub(result.len() as u64);
    Ok(result)
}
fn collect<R: Read + Seek>(
    input: R,
    depth: usize,
    budget: &mut usize,
    remaining: &mut u64,
    cancel: &AtomicBool,
    declared: &mut Declarations,
) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        return Err("TRIAL_COPY_CANCELLED".into());
    }
    if depth > 4 || *budget == 0 {
        return Err(ERROR.into());
    }
    *budget -= 1;
    let mut archive = zip::ZipArchive::new(input).map_err(|_| ERROR)?;
    if archive.len() > 20_000 {
        return Err(ERROR.into());
    }
    let (raw, quilt) = match bytes(&mut archive, "fabric.mod.json", 1024 * 1024, remaining) {
        Ok(raw) => (raw, false),
        Err(_) => (
            bytes(&mut archive, "quilt.mod.json", 1024 * 1024, remaining)?,
            true,
        ),
    };
    let value: Value = serde_json::from_slice(&raw).map_err(|_| ERROR)?;
    let node = if quilt {
        value.get("quilt_loader").ok_or(ERROR)?
    } else {
        &value
    };
    declared.ids.insert(id(&node["id"])?);
    if let Some(provides) = node.get("provides") {
        let list = provides.as_array().ok_or(ERROR)?;
        if list.len() > 64 {
            return Err(ERROR.into());
        }
        for alias in list {
            declared.ids.insert(id(if alias.is_string() {
                alias
            } else {
                &alias["id"]
            })?);
        }
    }
    if quilt {
        if let Some(deps) = node.get("depends") {
            let list = deps.as_array().ok_or(ERROR)?;
            if list.len() > 512 {
                return Err(ERROR.into());
            }
            for dep in list {
                if dep.get("unless").is_some()
                    || dep.get("any").is_some()
                    || dep.get("all").is_some()
                {
                    return Err(ERROR.into());
                }
                if dep["optional"] != true {
                    declared.required.insert(id(&dep["id"])?);
                }
            }
        }
    } else if let Some(deps) = node.get("depends") {
        let map = deps.as_object().ok_or(ERROR)?;
        if map.len() > 512 {
            return Err(ERROR.into());
        }
        for dep in map.keys() {
            declared.required.insert(id(&json!(dep))?);
        }
    }
    if let Some(jars) = node.get("jars").or_else(|| value.get("jars")) {
        let list = jars.as_array().ok_or(ERROR)?;
        if list.len() > 32 {
            return Err(ERROR.into());
        }
        for jar in list {
            let name = if jar.is_string() {
                jar.as_str()
            } else {
                jar["file"].as_str()
            }
            .ok_or(ERROR)?;
            if name.len() > 512
                || name.starts_with('/')
                || name.contains('\\')
                || name.split('/').any(|part| matches!(part, "" | "." | ".."))
            {
                return Err(ERROR.into());
            }
            let raw = bytes(&mut archive, name, 8 * 1024 * 1024, remaining)?;
            collect(
                Cursor::new(raw),
                depth + 1,
                budget,
                remaining,
                cancel,
                declared,
            )?;
        }
    }
    Ok(())
}
pub(super) fn complete(
    root: &Path,
    mut metadata: Value,
    cancel: &AtomicBool,
) -> Result<Value, String> {
    let items = metadata["items"].as_array_mut().ok_or(ERROR)?;
    let mut remaining = 64 * 1024 * 1024;
    for item in items.iter_mut().filter(|item| item["disabled"] == false) {
        let name = item["id"].as_str().ok_or(ERROR)?;
        if !super::basename(name) {
            return Err(ERROR.into());
        }
        let mut declared = Declarations::default();
        let mut budget = 64;
        collect(
            fs::File::open(root.join("mods").join(name)).map_err(|_| ERROR)?,
            0,
            &mut budget,
            &mut remaining,
            cancel,
            &mut declared,
        )?;
        item["declaredIds"] = json!(declared.ids);
        item["dependencies"] = json!(
            declared
                .required
                .into_iter()
                .map(|id| json!({"id":id,"optional":false}))
                .collect::<Vec<_>>()
        );
    }
    Ok(metadata)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn jar(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(vec![]));
        for (name, data) in entries {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn nested_mods_stay_in_parent_unit_with_their_required_edges() {
        let nested = jar(vec![(
            "fabric.mod.json",
            br#"{"id":"nested","depends":{"other":"*"}}"#.to_vec(),
        )]);
        let parent = jar(vec![
            (
                "fabric.mod.json",
                br#"{"id":"parent","jars":[{"file":"libs/nested.jar"}]}"#.to_vec(),
            ),
            ("libs/nested.jar", nested),
        ]);
        let mut declarations = Declarations::default();
        collect(
            Cursor::new(parent),
            0,
            &mut 64,
            &mut (64 * 1024 * 1024),
            &AtomicBool::new(false),
            &mut declarations,
        )
        .unwrap();
        assert_eq!(
            declarations.ids.into_iter().collect::<Vec<_>>(),
            vec!["nested", "parent"]
        );
        assert!(declarations.required.contains("other"));
    }
    #[test]
    fn unknown_and_complex_metadata_is_rejected() {
        assert!(
            collect(
                Cursor::new(jar(vec![])),
                0,
                &mut 64,
                &mut (64 * 1024 * 1024),
                &AtomicBool::new(false),
                &mut Declarations::default()
            )
            .is_err()
        );
        let quilt = jar(vec![(
            "quilt.mod.json",
            br#"{"quilt_loader":{"id":"test","depends":[{"any":[{"id":"one"},{"id":"two"}]}]}}"#
                .to_vec(),
        )]);
        assert!(
            collect(
                Cursor::new(quilt),
                0,
                &mut 64,
                &mut (64 * 1024 * 1024),
                &AtomicBool::new(false),
                &mut Declarations::default()
            )
            .is_err()
        );
    }
    #[test]
    fn cancellation_and_shared_decompression_budget_are_enforced() {
        let raw = jar(vec![("fabric.mod.json", br#"{"id":"test"}"#.to_vec())]);
        assert!(
            collect(
                Cursor::new(&raw),
                0,
                &mut 64,
                &mut 0,
                &AtomicBool::new(false),
                &mut Declarations::default()
            )
            .is_err()
        );
        assert_eq!(
            collect(
                Cursor::new(raw),
                0,
                &mut 64,
                &mut (64 * 1024 * 1024),
                &AtomicBool::new(true),
                &mut Declarations::default()
            )
            .unwrap_err(),
            "TRIAL_COPY_CANCELLED"
        );
    }
}
