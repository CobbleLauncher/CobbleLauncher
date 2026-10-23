// SPDX-License-Identifier: MIT
use super::*;
fn worlds(game: &Game) -> Result<Vec<cobble_content::LocalWorld>, String> {
    entries(&game.directory.join("saves"))?;
    cobble_content::list_local_worlds(&game.directory)
        .map_err(|_| "World metadata unavailable".into())
}
fn packs(items: Vec<cobble_content::LocalPack>, kind: &str) -> Vec<Value> {
    items.into_iter().map(|item| json!({"id":clean(&item.file_name),"name":clean(&item.name),"kind":kind,"sizeBytes":item.size_bytes,"packFormat":item.pack_format})).collect()
}
fn folder_packs(path: &Path, kind: &str) -> Result<Vec<Value>, String> {
    let mut items = vec![];
    for entry in entries(path)? {
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| "Pack metadata unavailable")?;
        if !metadata.is_dir() || linked(&metadata) {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !directory(&entry.path())? {
            continue;
        }
        let format = text(&entry.path().join("pack.mcmeta"))
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .and_then(|value| value.get("pack")?.get("pack_format")?.as_u64())
            .and_then(|value| u32::try_from(value).ok());
        items.push(json!({"id":clean(&name),"name":clean(&name),"kind":kind,"directory":true,"sizeBytes":null,"packFormat":format}));
    }
    Ok(items)
}
pub(super) fn collect(game: &Game, scope: &str) -> Result<Value, String> {
    let mut result = match scope {
        "mods" => {
            entries(&game.directory.join("mods"))?;
            let items = cobble_content::list_mods(&game.directory)
                .map_err(|_| "Mod metadata unavailable")?;
            json!({"items":items.into_iter().map(|item| {let (declared_ids,dependencies)=mods::declarations(&game.directory,&item.file_name);json!({"id":clean(&item.file_name),"name":clean(&item.name),"version":item.version.map(|v|clean(&v)),"loader":item.loader.map(|v|clean(&v)),"disabled":!item.enabled,"declaredIds":declared_ids,"dependencies":dependencies})}).collect::<Vec<_>>()})
        }
        "packs" => {
            entries(&game.directory.join("resourcepacks"))?;
            entries(&game.directory.join("shaderpacks"))?;
            let mut items = packs(
                cobble_content::list_local_packs(
                    &game.directory,
                    cobble_content::LocalPackKind::ResourcePack,
                )
                .map_err(|_| "Pack metadata unavailable")?,
                "resource_pack",
            );
            items.extend(packs(
                cobble_content::list_local_packs(
                    &game.directory,
                    cobble_content::LocalPackKind::ShaderPack,
                )
                .map_err(|_| "Pack metadata unavailable")?,
                "shader_pack",
            ));
            items.extend(folder_packs(
                &game.directory.join("resourcepacks"),
                "resource_pack",
            )?);
            items.extend(folder_packs(
                &game.directory.join("shaderpacks"),
                "shader_pack",
            )?);
            if items.len() > LIMIT {
                return Err("Pack entry limit exceeded".into());
            }
            let selected = settings::selected_packs(&game.directory);
            json!({"items":items,"selectedResourcePacksAvailable":selected.is_some(),"selectedResourcePacks":selected.unwrap_or_default()})
        }
        "worlds" => {
            json!({"items":worlds(game)?.into_iter().map(|item| json!({"id":clean(&item.folder_name),"name":clean(&item.name),"version":item.minecraft_version.map(|v|clean(&v)),"lastPlayed":item.last_played,"gameMode":item.game_mode,"hardcore":item.hardcore,"metadataError":item.metadata_error})).collect::<Vec<_>>(),"inspectedAt":now()})
        }
        "backups" => {
            entries(&game.directory.join(".cobble-world-backups"))?;
            let items = cobble_core::list_world_replacement_backups(&game.directory)
                .map_err(|_| "Backup index unavailable")?;
            json!({"items":items.into_iter().map(|item|json!({"id":clean(&item.backup_id),"worldId":clean(&item.folder_name),"name":clean(&item.world_name),"createdAt":item.created_at,"kind":"replacement"})).collect::<Vec<_>>(),"worlds":worlds(game)?.into_iter().map(|world|json!({"id":clean(&world.folder_name),"name":clean(&world.name)})).collect::<Vec<_>>(),"inspectedAt":now(),"encryptedSnapshots":"not_inspected"})
        }
        "datapacks" => {
            let mut items = vec![];
            let world_list = worlds(game)?;
            if world_list.len() > 32 {
                return Err("Datapack world limit exceeded".into());
            }
            for world in world_list {
                let root = game.directory.join("saves").join(&world.folder_name);
                entries(&root.join("datapacks"))?;
                entries(&root.join(".cobble-removed-datapacks"))?;
                let found =
                    cobble_content::list_world_datapacks(&game.directory, &world.folder_name)
                        .map_err(|_| "Datapack metadata unavailable")?;
                for (list, removed) in [(found.installed, false), (found.removed, true)] {
                    for item in list {
                        if items.len() >= LIMIT {
                            return Err("Datapack entry limit exceeded".into());
                        }
                        items.push(json!({"worldId":clean(&world.folder_name),"id":clean(&item.file_name),"name":clean(&item.name),"removed":removed,"sizeBytes":item.size_bytes,"packFormat":item.pack_format}));
                    }
                }
                for (folder, removed) in [("datapacks", false), (".cobble-removed-datapacks", true)]
                {
                    for mut item in folder_packs(&root.join(folder), "datapack")? {
                        if items.len() >= LIMIT {
                            return Err("Datapack entry limit exceeded".into());
                        }
                        item["worldId"] = json!(clean(&world.folder_name));
                        item["removed"] = json!(removed);
                        items.push(item);
                    }
                }
            }
            json!({"items":items})
        }
        _ => return Err("Unsupported content scope".into()),
    };
    result["available"] = json!(true);
    result["truncated"] = json!(false);
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_directory_resource_pack_is_present_with_bounded_metadata() {
        let root = tempfile::tempdir().unwrap();
        let packs = root.path().join("resourcepacks");
        let folder = packs.join("FolderPack");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("pack.mcmeta"),
            br#"{"pack":{"pack_format":34,"description":"not exposed"}}"#,
        )
        .unwrap();
        fs::write(
            root.path().join("options.txt"),
            "resourcePacks:[\"vanilla\",\"file/FolderPack\"]\n",
        )
        .unwrap();
        let items = folder_packs(&packs, "resource_pack").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "FolderPack");
        assert_eq!(items[0]["packFormat"], 34);
        assert_eq!(items[0]["directory"], true);
        assert!(items[0]["sizeBytes"].is_null());
        assert!(items[0].get("description").is_none());
        let selected = settings::selected_packs(root.path()).unwrap();
        for id in selected.iter().filter_map(|id| id.strip_prefix("file/")) {
            assert!(items.iter().any(|item| item["id"] == id));
        }
    }
    #[test]
    fn folder_pack_with_unknown_metadata_keeps_nullable_format() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("NoMetadata")).unwrap();
        let items = folder_packs(root.path(), "datapack").unwrap();
        assert!(items[0]["packFormat"].is_null());
    }
}
