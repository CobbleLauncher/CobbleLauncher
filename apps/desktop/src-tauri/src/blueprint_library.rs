// SPDX-License-Identifier: MIT
//! Library metadata edits never modify schematic contents or imported game copies.
use super::{Blueprint, list_blueprints, read_file, root, valid_id};
use std::{fs, io::Write, path::Path, sync::Mutex};

static LIBRARY_WRITES: Mutex<()> = Mutex::new(());

#[tauri::command]
pub fn save_blueprint_thumbnail(id: String, png: String) -> Result<(), String> {
    valid_id(&id)?;
    let bytes = thumbnail_png(&png)?;
    let _guard = LIBRARY_WRITES.lock().map_err(|error| error.to_string())?;
    let directory = root("blueprints")?;
    let mut blueprint = metadata(&directory, &id)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(&directory).map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(directory.join(format!("{id}.png")))
        .map_err(|error| error.to_string())?;
    blueprint.has_thumbnail = true;
    write_metadata(&directory, &id, &blueprint)
}

fn thumbnail_png(png: &str) -> Result<Vec<u8>, String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    if png.len() > 512 * 1024 {
        return Err("蓝图缩略图过大".into());
    }
    let bytes = STANDARD
        .decode(
            png.strip_prefix("data:image/png;base64,")
                .ok_or("需要 PNG 缩略图")?,
        )
        .map_err(|_| "无效缩略图")?;
    if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png) {
        return Err("需要 PNG 缩略图".into());
    }
    crate::games::game_appearance::normalize_size(&bytes, 320, 180)
}

#[tauri::command]
pub fn read_blueprint_thumbnail(id: String) -> Result<Option<String>, String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let directory = root("blueprints")?;
    if !metadata(&directory, &id)?.has_thumbnail {
        return Ok(None);
    }
    let path = directory.join(format!("{id}.png"));
    if fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len()
        > 1024 * 1024
    {
        return Err("蓝图缩略图过大".into());
    }
    let bytes = read_file(&path)?;
    let bytes = crate::games::game_appearance::normalize_size(&bytes, 320, 180)?;
    Ok(Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(bytes)
    )))
}

#[tauri::command]
pub fn set_blueprint_favorite(id: String, favorite: bool) -> Result<Vec<Blueprint>, String> {
    let _guard = LIBRARY_WRITES.lock().map_err(|error| error.to_string())?;
    favorite_in(&root("blueprints")?, &id, favorite)?;
    list_blueprints()
}

fn favorite_in(directory: &Path, id: &str, favorite: bool) -> Result<(), String> {
    let mut blueprint = metadata(directory, id)?;
    blueprint.favorite = favorite;
    write_metadata(directory, id, &blueprint)
}

#[tauri::command]
pub fn rename_blueprint(id: String, name: String) -> Result<Vec<Blueprint>, String> {
    let _guard = LIBRARY_WRITES.lock().map_err(|error| error.to_string())?;
    rename_in(&root("blueprints")?, &id, &name)?;
    tracing::info!(event = "blueprint_renamed", blueprint_id = %id);
    list_blueprints()
}

#[tauri::command]
pub fn remove_blueprint(id: String) -> Result<Vec<Blueprint>, String> {
    let _guard = LIBRARY_WRITES.lock().map_err(|error| error.to_string())?;
    remove_in(&root("blueprints")?, &id)?;
    tracing::info!(event = "blueprint_removed", blueprint_id = %id);
    list_blueprints()
}

fn metadata(directory: &Path, id: &str) -> Result<Blueprint, String> {
    valid_id(id)?;
    let blueprint: Blueprint =
        serde_json::from_slice(&read_file(&directory.join(format!("{id}.json")))?)
            .map_err(|error| error.to_string())?;
    if blueprint.id != id {
        return Err("蓝图标识不匹配".into());
    }
    Ok(blueprint)
}

fn rename_in(directory: &Path, id: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err("蓝图名称需为 1–120 个字符，且不能包含控制字符".into());
    }
    let mut blueprint = metadata(directory, id)?;
    blueprint.name = name.into();
    write_metadata(directory, id, &blueprint)
}

fn write_metadata(directory: &Path, id: &str, blueprint: &Blueprint) -> Result<(), String> {
    let bytes = serde_json::to_vec(&blueprint).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(directory.join(format!("{id}.json")))
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn remove_in(directory: &Path, id: &str) -> Result<(), String> {
    let blueprint = metadata(directory, id)?;
    match fs::remove_file(directory.join(format!("{id}.png"))) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    // Only this item's fixed library entries are eligible. Missing schematic data
    // does not prevent removing its stale library metadata.
    match fs::remove_file(directory.join(format!("{id}.{}", blueprint.format.extension()))) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    fs::remove_file(directory.join(format!("{id}.json"))).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_metadata_defaults_to_schem_and_unknown_formats_are_rejected() {
        let legacy = br#"{"id":"old","name":"Old","pluginId":"viewer"}"#;
        let value: Blueprint = serde_json::from_slice(legacy).unwrap();
        assert_eq!(value.format, super::super::BlueprintFormat::Schem);
        assert!(!value.favorite);
        assert!(!value.has_thumbnail);
        assert!(
            serde_json::from_str::<Blueprint>(
                r#"{"id":"old","name":"Old","pluginId":"viewer","format":"../escape"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn thumbnails_are_bounded_decoded_and_normalized() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use std::io::Cursor;
        let mut source = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(16, 16)
            .write_to(&mut source, image::ImageFormat::Png)
            .unwrap();
        let png = format!(
            "data:image/png;base64,{}",
            STANDARD.encode(source.into_inner())
        );
        let normalized = thumbnail_png(&png).unwrap();
        let decoded = image::load_from_memory(&normalized).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (320, 180));
        assert!(thumbnail_png("data:image/png;base64,bad").is_err());
        assert!(thumbnail_png("data:image/svg+xml;base64,PHN2Zz4=").is_err());
        assert!(thumbnail_png(&"x".repeat(512 * 1024 + 1)).is_err());
    }

    #[test]
    fn rename_and_remove_preserve_litematic_format() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("saved.json"),
            br#"{"id":"saved","name":"Before.litematic","pluginId":"viewer","format":"litematic"}"#,
        )
        .unwrap();
        fs::write(root.path().join("saved.litematic"), b"original").unwrap();
        fs::write(root.path().join("saved.schem"), b"unrelated").unwrap();
        rename_in(root.path(), "saved", "Renamed").unwrap();
        assert_eq!(
            metadata(root.path(), "saved").unwrap().format,
            super::super::BlueprintFormat::Litematic
        );
        assert_eq!(
            fs::read(root.path().join("saved.litematic")).unwrap(),
            b"original"
        );
        remove_in(root.path(), "saved").unwrap();
        assert!(!root.path().join("saved.litematic").exists());
        assert_eq!(
            fs::read(root.path().join("saved.schem")).unwrap(),
            b"unrelated"
        );
    }

    fn seed(directory: &Path, id: &str) {
        let blueprint = Blueprint {
            id: id.into(),
            name: "Original.schem".into(),
            plugin_id: "schematic-viewer".into(),
            format: super::super::BlueprintFormat::Schem,
            source: None,
            favorite: false,
            has_thumbnail: false,
        };
        fs::write(
            directory.join(format!("{id}.json")),
            serde_json::to_vec(&blueprint).unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join(format!("{id}.schem")),
            b"unchanged schematic bytes",
        )
        .unwrap();
    }

    #[test]
    fn rename_retains_online_origin_and_does_not_turn_attribution_into_a_license() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("online.json"), br#"{"id":"online","name":"Original","pluginId":"viewer","format":"schem","source":{"provider":"mcs-provider","id":"11111111-1111-4111-8111-111111111111","version":1,"author":"Synthetic Author","attribution":"repost","license":null}}"#).unwrap();
        rename_in(root.path(), "online", "Renamed").unwrap();
        let source = metadata(root.path(), "online").unwrap().source.unwrap();
        assert_eq!(source.provider, "mcs-provider");
        assert_eq!(source.author, "Synthetic Author");
        assert_eq!(source.attribution, "repost");
        assert_eq!(source.license, None);
    }

    #[test]
    fn rename_changes_only_display_metadata() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), "saved-1");
        rename_in(root.path(), "saved-1", "  中文小屋  ").unwrap();
        let blueprint = metadata(root.path(), "saved-1").unwrap();
        assert_eq!(blueprint.name, "中文小屋");
        assert_eq!(blueprint.plugin_id, "schematic-viewer");
        assert_eq!(
            fs::read(root.path().join("saved-1.schem")).unwrap(),
            b"unchanged schematic bytes"
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn favorite_survives_rename_and_never_changes_schematic_bytes() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), "saved-1");
        favorite_in(root.path(), "saved-1", true).unwrap();
        rename_in(root.path(), "saved-1", "Favorite").unwrap();
        assert!(metadata(root.path(), "saved-1").unwrap().favorite);
        assert_eq!(
            fs::read(root.path().join("saved-1.schem")).unwrap(),
            b"unchanged schematic bytes"
        );
        favorite_in(root.path(), "saved-1", false).unwrap();
        assert!(!metadata(root.path(), "saved-1").unwrap().favorite);
        assert!(favorite_in(root.path(), "../saved-1", true).is_err());
    }

    #[test]
    fn remove_preserves_other_blueprints_and_imported_copies() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), "saved-1");
        seed(root.path(), "saved-2");
        let game = root.path().join("schematics");
        fs::create_dir(&game).unwrap();
        fs::copy(
            root.path().join("saved-1.schem"),
            game.join("imported.schem"),
        )
        .unwrap();
        remove_in(root.path(), "saved-1").unwrap();
        assert!(!root.path().join("saved-1.json").exists());
        assert!(!root.path().join("saved-1.schem").exists());
        assert!(root.path().join("saved-2.json").is_file());
        assert_eq!(
            fs::read(game.join("imported.schem")).unwrap(),
            b"unchanged schematic bytes"
        );
    }

    #[test]
    fn rejects_invalid_ids_and_names_without_mutation() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), "saved-1");
        let before = fs::read(root.path().join("saved-1.json")).unwrap();
        for id in ["../saved-1", "saved-1/../../outside", "", "SAVED-1"] {
            assert!(rename_in(root.path(), id, "Changed").is_err());
            assert!(remove_in(root.path(), id).is_err());
        }
        for name in [" ".to_owned(), "bad\nname".to_owned(), "a".repeat(121)] {
            assert!(rename_in(root.path(), "saved-1", &name).is_err());
        }
        assert_eq!(fs::read(root.path().join("saved-1.json")).unwrap(), before);
        assert!(root.path().join("saved-1.schem").is_file());
    }

    #[test]
    fn removes_stale_entry_with_missing_schematic() {
        let root = tempfile::tempdir().unwrap();
        seed(root.path(), "saved-1");
        fs::remove_file(root.path().join("saved-1.schem")).unwrap();
        remove_in(root.path(), "saved-1").unwrap();
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}
