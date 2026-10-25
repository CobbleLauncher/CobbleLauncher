// SPDX-License-Identifier: MIT
//! User-selected text output. Plugin requests never carry or receive filesystem paths.
use super::plugin_views::ViewAuthorization;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRequest {
    pub name: String,
    pub text: String,
}
fn validate(args: &TextRequest) -> Result<String, String> {
    if args.name.trim().is_empty()
        || args.name.chars().count() > 96
        || args.name.starts_with(['.', ' '])
        || args.name.ends_with(['.', ' '])
        || args
            .name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err("PLUGIN_EXPORT_NAME_INVALID".into());
    }
    let stem = args
        .name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("PLUGIN_EXPORT_NAME_INVALID".into());
    }
    if args.text.len() > 1_048_576 || args.text.contains('\0') {
        return Err("PLUGIN_EXPORT_TEXT_INVALID".into());
    }
    Ok(if args.name.to_ascii_lowercase().ends_with(".txt") {
        args.name.clone()
    } else {
        format!("{}.txt", args.name)
    })
}
pub async fn text(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: TextRequest,
) -> Result<Value, String> {
    let name = validate(&args)?;
    let window = app
        .get_webview_window("main")
        .ok_or("PLUGIN_EXPORT_UNAVAILABLE")?;
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_file_name(name)
        .add_filter("Text", &["txt"])
        .save_file()
        .await
    else {
        return Ok(json!({"saved":false}));
    };
    let cancel = auth.cancel.clone();
    let path = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        if cancel.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        let parent = path.parent().ok_or("PLUGIN_EXPORT_UNAVAILABLE")?;
        let mut temp =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| "PLUGIN_EXPORT_UNAVAILABLE")?;
        use std::io::Write;
        temp.write_all(args.text.as_bytes())
            .map_err(|_| "PLUGIN_EXPORT_UNAVAILABLE")?;
        temp.as_file()
            .sync_all()
            .map_err(|_| "PLUGIN_EXPORT_UNAVAILABLE")?;
        if cancel.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        temp.persist(&path)
            .map_err(|_| "PLUGIN_EXPORT_UNAVAILABLE")?;
        Ok(json!({"saved":true}))
    })
    .await
    .map_err(|_| "PLUGIN_EXPORT_UNAVAILABLE")?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_text_export_does_not_accept_paths_or_reserved_names() {
        for name in [
            "../secret",
            "C:\\file",
            "CON.txt",
            "nul",
            "LPT1.txt",
            "bad.",
        ] {
            assert!(
                validate(&TextRequest {
                    name: name.into(),
                    text: String::new()
                })
                .is_err()
            )
        }
        assert_eq!(
            validate(&TextRequest {
                name: "redacted-log".into(),
                text: "a\nb".into()
            })
            .unwrap(),
            "redacted-log.txt"
        );
    }
    #[test]
    fn export_preserves_full_megabyte_and_rejects_larger_or_nul() {
        assert!(
            validate(&TextRequest {
                name: "log".into(),
                text: "a".repeat(1_048_576)
            })
            .is_ok()
        );
        for text in ["a".repeat(1_048_577), "a\0b".into()] {
            assert!(
                validate(&TextRequest {
                    name: "log".into(),
                    text
                })
                .is_err()
            )
        }
    }
}
