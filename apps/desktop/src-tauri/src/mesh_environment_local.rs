// SPDX-License-Identifier: MIT
//! Only receiver-local registered games can supply personal controls/graphics.
use super::*;
use sha2::{Digest, Sha256};
fn revision(source: &str, target: &str, options_revision: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{source}\0{target}\0{options_revision}"))
    )
}
#[derive(Clone)]
pub(super) struct LocalOptions {
    pub source_game_id: Option<String>,
    pub revision: String,
    pub options: Option<cobble_core::personal_options::PersonalOptions>,
    pub summary: Value,
}
pub(super) async fn capture(
    app: &AppHandle,
    game_id: Option<&str>,
    target_version: &str,
) -> LocalOptions {
    let unavailable = |reason: &str| LocalOptions {
        source_game_id: game_id.map(str::to_owned),
        revision: reason.into(),
        options: None,
        summary: json!({"keys":0,"graphics":0,"skipped":0,"reason":reason}),
    };
    let Some(game_id) = game_id else {
        return unavailable("noSource");
    };
    let Ok(game) = crate::games::workspace::snapshot(app, game_id) else {
        return unavailable("unavailable");
    };
    let target = target_version.to_owned();
    let source_version = game.version.clone();
    let name = game.name;
    let result = tauri::async_runtime::spawn_blocking(move || {
        cobble_core::personal_options::capture(&game.directory, &game.version, &target)
    })
    .await;
    match result {
        Ok(Ok(captured)) => LocalOptions {
            source_game_id: Some(game_id.into()),
            revision: revision(&source_version, target_version, &captured.revision),
            options: captured.options,
            summary: json!({"sourceName":name,"keys":captured.keys,"graphics":captured.graphics,"skipped":captured.skipped,"reason":captured.reason}),
        },
        _ => unavailable("unavailable"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_options_revision_rejects_source_version_or_content_changes() {
        let original = revision("1.21.1", "1.21.1", "source-options-hash");
        assert_eq!(
            original,
            revision("1.21.1", "1.21.1", "source-options-hash")
        );
        assert_ne!(
            original,
            revision("1.21.4", "1.21.1", "source-options-hash")
        );
        assert_ne!(
            original,
            revision("1.21.1", "1.21.1", "edited-options-hash")
        );
        assert_ne!(
            original,
            revision("1.21.1", "1.21.4", "source-options-hash")
        );
    }
}
