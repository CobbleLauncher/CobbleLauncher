// SPDX-License-Identifier: MIT
use std::path::PathBuf;
use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
struct Feedback {
    active: bool,
    kind: &'static str,
    count: usize,
}

fn classify(paths: &[PathBuf]) -> Feedback {
    let kinds: Vec<_> = paths
        .iter()
        .map(|path| {
            match path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "mrpack" => "mrpack",
                "zip" => "archive",
                _ => "other",
            }
        })
        .collect();
    let kind = if kinds.is_empty() || kinds.iter().all(|kind| *kind == "other") {
        "none"
    } else if kinds.iter().all(|kind| *kind == "mrpack") {
        "mrpack"
    } else if kinds.iter().all(|kind| *kind == "archive") {
        "archive"
    } else {
        "mixed"
    };
    Feedback {
        active: kind != "none",
        kind,
        count: paths.len(),
    }
}

pub fn handle(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() != "main" {
        return;
    }
    let feedback = match event {
        tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Enter { paths, .. }) => classify(paths),
        // Over has no paths; retain the classification from Enter in the renderer.
        tauri::WindowEvent::DragDrop(
            tauri::DragDropEvent::Leave | tauri::DragDropEvent::Drop { .. },
        )
        | tauri::WindowEvent::Focused(false)
        | tauri::WindowEvent::Destroyed => Feedback {
            active: false,
            kind: "none",
            count: 0,
        },
        _ => return,
    };
    let _ = window.emit("cobble-pack-drag", feedback);
}

/// Only native OS-approved paths enter here. Existing adapters pin their own archive
/// before creating a preview; no download or installation occurs.
pub fn preview(
    app: &tauri::AppHandle,
    paths: &[PathBuf],
) -> Result<Option<serde_json::Value>, String> {
    let [path] = paths else {
        return Ok(None);
    };
    let Some(kind) = detect(path) else {
        return Ok(None);
    };
    let preview = if kind == "mrpack" {
        crate::games::file_inbox_games::mrpack(app, path)?
    } else {
        crate::games::file_inbox_games::curseforge(app, path)?
    };
    Ok(Some(serde_json::json!({"kind":kind,"preview":preview})))
}

fn detect(path: &std::path::Path) -> Option<&'static str> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = match extension.as_str() {
        "mrpack" => "mrpack",
        "zip" if cobble_content::read_mrpack(path).is_ok() => "mrpack",
        "zip" if cobble_content::read_curseforge_pack(path).is_ok() => "curseforge-pack",
        _ => return None,
    };
    Some(kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn recognizes_manifests_and_preserves_other_zip_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        for (name, content, expected) in [
            (
                "modrinth.index.json",
                r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Fixture","dependencies":{"minecraft":"1.21.1"},"files":[]}"#,
                Some("mrpack"),
            ),
            (
                "manifest.json",
                r#"{"manifestType":"minecraftModpack","manifestVersion":1,"name":"Fixture","version":"1","overrides":"overrides","minecraft":{"version":"1.20.1","modLoaders":[]},"files":[]}"#,
                Some("curseforge-pack"),
            ),
            (
                "pack.mcmeta",
                r#"{"pack":{"pack_format":34,"description":"Fixture"}}"#,
                None,
            ),
        ] {
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content.as_bytes()).unwrap();
            writer.finish().unwrap();
            assert_eq!(detect(&path), expected);
        }
        let invalid = dir.path().join("invalid.mrpack");
        std::fs::write(&invalid, b"not an archive").unwrap();
        assert_eq!(detect(&invalid), Some("mrpack"));
        assert!(cobble_content::read_mrpack(&invalid).is_err());
    }
    #[test]
    fn only_extensions_leave_host_and_zip_is_only_an_archive() {
        let feedback = classify(&[PathBuf::from("private/PACK.ZIP")]);
        assert_eq!(feedback.kind, "archive");
        assert!(feedback.active);
        let json = serde_json::to_string(&feedback).unwrap();
        assert!(!json.contains("private"));
        assert!(!json.contains("PACK"));
        assert_eq!(
            classify(&["a.mrpack".into(), "b.schem".into()]).kind,
            "mixed"
        );
        assert!(!classify(&["a.schem".into()]).active);
        assert!(!classify(&[]).active);
    }
}
