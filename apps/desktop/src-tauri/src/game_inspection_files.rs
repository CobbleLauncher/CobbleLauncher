// SPDX-License-Identifier: MIT
use super::*;
const VISIT_LIMIT: usize = 10_000;
#[derive(Default)]
struct Totals {
    bytes: u64,
    files: u64,
    truncated: bool,
}
fn size(path: &Path, budget: &mut usize, depth: usize) -> Totals {
    let mut total = Totals::default();
    match directory(path) {
        Ok(false) => return total,
        Err(_) => {
            total.truncated = true;
            return total;
        }
        Ok(true) => {}
    }
    if depth > 12 || *budget == 0 {
        total.truncated = true;
        return total;
    }
    let Ok(children) = fs::read_dir(path) else {
        total.truncated = true;
        return total;
    };
    for entry in children {
        if *budget == 0 {
            total.truncated = true;
            break;
        }
        *budget -= 1;
        let Ok(entry) = entry else {
            total.truncated = true;
            continue;
        };
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            total.truncated = true;
            continue;
        };
        if linked(&metadata) {
            total.truncated = true;
            continue;
        }
        if metadata.is_file() {
            total.bytes = total.bytes.saturating_add(metadata.len());
            total.files += 1;
        } else if metadata.is_dir() {
            let nested = size(&entry.path(), budget, depth + 1);
            total.bytes = total.bytes.saturating_add(nested.bytes);
            total.files += nested.files;
            total.truncated |= nested.truncated;
        }
    }
    total
}
fn screenshots(path: &Path) -> Result<Value, String> {
    if !directory(path)? {
        return Ok(json!({"items":[],"truncated":false,"available":true,"inspectedAt":now()}));
    }
    let children = fs::read_dir(path).map_err(|_| "Screenshot directory unavailable")?;
    let mut items = vec![];
    let mut truncated = false;
    for (visited, entry) in children.enumerate() {
        if visited >= VISIT_LIMIT {
            truncated = true;
            break;
        }
        let Ok(entry) = entry else {
            truncated = true;
            continue;
        };
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            truncated = true;
            continue;
        };
        if linked(&metadata) {
            truncated = true;
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        if !metadata.is_file()
            || !extension
                .is_some_and(|value| matches!(value.as_str(), "png" | "jpg" | "jpeg" | "webp"))
        {
            continue;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|time| time.as_secs());
        items.push(json!({"id":clean(&name),"name":clean(&name),"sizeBytes":metadata.len(),"modifiedAt":modified}));
    }
    items.sort_by(|left, right| {
        right["modifiedAt"]
            .as_u64()
            .cmp(&left["modifiedAt"].as_u64())
            .then_with(|| left["id"].as_str().cmp(&right["id"].as_str()))
    });
    truncated |= items.len() > LIMIT;
    items.truncate(LIMIT);
    Ok(json!({"items":items,"truncated":truncated,"available":true,"inspectedAt":now()}))
}
pub(super) fn collect(game: &Game, scope: &str) -> Result<Value, String> {
    if scope == "screenshots" {
        return screenshots(&game.directory.join("screenshots"));
    }
    let mut items = vec![];
    let mut budget = VISIT_LIMIT;
    let mut truncated = false;
    for folder in [
        "mods",
        "resourcepacks",
        "shaderpacks",
        "saves",
        "screenshots",
        "logs",
        "config",
    ] {
        let total = size(&game.directory.join(folder), &mut budget, 0);
        truncated |= total.truncated;
        items.push(json!({"id":folder,"name":folder,"sizeBytes":total.bytes,"fileCount":total.files,"truncated":total.truncated}));
    }
    Ok(json!({"items":items,"truncated":truncated,"available":true,"inspectedAt":now()}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_folder_keeps_bounded_partial_totals() {
        let root = tempfile::tempdir().unwrap();
        for n in 0..600 {
            fs::write(root.path().join(n.to_string()), b"abc").unwrap();
        }
        let mut budget = VISIT_LIMIT;
        let complete = size(root.path(), &mut budget, 0);
        assert_eq!(complete.files, 600);
        assert_eq!(complete.bytes, 1800);
        assert!(!complete.truncated);
        let mut budget = 100;
        let partial = size(root.path(), &mut budget, 0);
        assert_eq!(partial.files, 100);
        assert_eq!(partial.bytes, 300);
        assert!(partial.truncated);
    }
    #[test]
    fn screenshot_output_is_capped_without_losing_available_inventory() {
        let root = tempfile::tempdir().unwrap();
        for n in 0..600 {
            fs::write(root.path().join(format!("{n:04}.png")), b"metadata only").unwrap();
        }
        let result = screenshots(root.path()).unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(), LIMIT);
        assert_eq!(result["truncated"], true);
        assert_eq!(result["available"], true);
    }
}
