// SPDX-License-Identifier: MIT
use super::*;
use crate::space::{SpaceVolume, estimate_space};
use sha2::Sha512;
use std::collections::BTreeMap;
fn volumes(cache: u64, target: u64) -> [SpaceVolume; 2] {
    [
        SpaceVolume {
            id: "cache".into(),
            label: "cache".into(),
            available_bytes: Some(cache),
        },
        SpaceVolume {
            id: "target".into(),
            label: "target".into(),
            available_bytes: Some(target),
        },
    ]
}
#[test]
fn standalone_java_archive_accounts_cache_transfer_staging_and_runtime_copy_before_unknown_extraction()
 {
    let uncached = java_archive_demand(40, false, "cache", "target");
    let plan = estimate_space(&volumes(79, 1000), &[uncached], &BTreeMap::new(), 1).unwrap();
    assert!(!plan.can_proceed);
    assert_eq!(plan.volumes[0].download_bytes, 40);
    assert_eq!(plan.volumes[0].download_staging_bytes, 40);
    assert_eq!(plan.volumes[1].extraction_scratch_bytes, 40);
    assert!(plan.volumes[1].unknown_sizes > 0);
    let cached = estimate_space(
        &volumes(0, 40),
        &[java_archive_demand(40, true, "cache", "target")],
        &BTreeMap::new(),
        1,
    )
    .unwrap();
    assert!(cached.can_proceed && !cached.exact);
    assert_eq!(cached.volumes[0].additional_peak_bytes, 0);
    let short = estimate_space(
        &volumes(0, 39),
        &[java_archive_demand(40, true, "cache", "target")],
        &BTreeMap::new(),
        1,
    )
    .unwrap();
    assert!(!short.can_proceed);
}
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cobble-pack-space-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn file(path: &str, bytes: &[u8], optional: bool) -> cobble_content::MrpackFile {
    cobble_content::MrpackFile {
        path: path.into(),
        downloads: vec!["https://cdn.modrinth.com/data/fixture.jar".into()],
        file_size: bytes.len() as u64,
        sha1: format!("{:x}", Sha1::digest(bytes)),
        sha512: format!("{:x}", Sha512::digest(bytes)),
        optional,
    }
}
#[test]
fn curseforge_pack_space_counts_verified_manual_cache_and_zip_expansion_before_writes() {
    let temp = Temp::new();
    let cache = ModDownloads::open_at(
        &temp.0.join("cache"),
        crate::modrinth::ModProvider::Modrinth,
        None,
    )
    .unwrap();
    let selected = cobble_content::ModrinthFile {
        url: String::new(),
        filename: "manual.jar".into(),
        size: 8,
        sha1: format!("{:x}", Sha1::digest(b"required")),
        sha512: None,
    };
    let manual = temp.0.join("manual.jar");
    std::fs::write(&manual, b"required").unwrap();
    cache
        .ingest_manual(&selected, std::fs::File::open(manual).unwrap())
        .unwrap();
    let pack = cobble_content::CurseForgePack {
        name: "fixture".into(),
        version: "1".into(),
        minecraft_version: "1.21.1".into(),
        loader: None,
        files: vec![],
        overrides: vec![cobble_content::PackOverride {
            entry_index: 0,
            path: "config/a.json".into(),
            layer: 0,
            size: 5,
        }],
    };
    let plan = crate::CurseForgePackPlan {
        archive_sha256: "a".repeat(64),
        optional_files: vec![],
        files: cobble_content::curseforge::PackInstallPlan {
            files: vec![cobble_content::curseforge::ResolvedPackFile {
                project_id: "1".into(),
                file_id: "2".into(),
                file: selected,
                destination: cobble_content::curseforge::PackFileDestination::Mods,
                name: "fixture".into(),
                version_number: "1".into(),
                required_by: None,
            }],
            restricted: vec![],
            incompatible: vec![],
        },
    };
    let control = OperationControl::new(std::sync::Arc::new(|_| {}));
    let artifacts = curseforge::artifacts(
        &pack,
        &plan,
        vec!["mods/manual.jar".into()],
        &temp.0.join("target"),
        "cache",
        "target",
        &cache,
        &control,
    )
    .unwrap();
    assert!(artifacts[0].verified_cached);
    let demand = estimate_space(&volumes(10, 12), &artifacts, &Default::default(), 4).unwrap();
    assert_eq!(demand.volumes[0].additional_peak_bytes, 10);
    assert_eq!(demand.volumes[1].materialized_bytes, 13);
    assert!(!demand.can_proceed);
    assert!(!temp.0.join("target").exists());
    let empty_cache = ModDownloads::open_at(
        &temp.0.join("empty-cache"),
        crate::modrinth::ModProvider::Modrinth,
        None,
    )
    .unwrap();
    std::fs::create_dir_all(temp.0.join("target/mods")).unwrap();
    std::fs::write(temp.0.join("target/mods/manual.jar"), b"required").unwrap();
    let retry = curseforge::artifacts(
        &pack,
        &plan,
        vec!["mods/manual.jar".into()],
        &temp.0.join("target"),
        "cache",
        "target",
        &empty_cache,
        &control,
    )
    .unwrap();
    assert!(!retry[0].verified_cached && !retry[0].already_installed);
    assert!(retry[1].already_installed);
    let retry_space = estimate_space(&volumes(0, 100), &retry, &Default::default(), 4).unwrap();
    assert!(!retry_space.can_proceed);
    assert_eq!(retry_space.volumes[0].download_bytes, 13);
    assert_eq!(retry_space.volumes[1].materialized_bytes, 5);
    control.cancel();
    assert!(
        curseforge::artifacts(
            &pack,
            &plan,
            vec!["mods/manual.jar".into()],
            &temp.0.join("target"),
            "cache",
            "target",
            &cache,
            &control
        )
        .is_err()
    );
}
#[test]
fn mrpack_space_uses_verified_cache_optional_choices_and_declared_override_expansion() {
    let temp = Temp::new();
    let cache = ModDownloads::open_at(
        &temp.0.join("cache"),
        crate::modrinth::ModProvider::Modrinth,
        None,
    )
    .unwrap();
    let required = file("mods/required.jar", b"required", false);
    let selected = temp.0.join("required.jar");
    std::fs::write(&selected, b"required").unwrap();
    cache
        .ingest_manual(
            &cobble_content::ModrinthFile {
                url: required.downloads[0].clone(),
                filename: "required.jar".into(),
                size: required.file_size,
                sha1: required.sha1.clone(),
                sha512: Some(required.sha512.clone()),
            },
            std::fs::File::open(selected).unwrap(),
        )
        .unwrap();
    let pack = cobble_content::Mrpack {
        name: "fixture".into(),
        version_id: "v1".into(),
        summary: None,
        minecraft_version: "1.21.1".into(),
        loader_kind: None,
        loader_version: None,
        files: vec![required, file("mods/optional.jar", b"optional", true)],
        overrides: vec![cobble_content::PackOverride {
            entry_index: 0,
            path: "config/a.txt".into(),
            layer: 0,
            size: 5,
        }],
    };
    let control = OperationControl::new(std::sync::Arc::new(|_| {}));
    let artifacts = pack_artifacts(
        &pack,
        &[],
        &temp.0.join("target"),
        "cache",
        "target",
        &cache,
        &control,
    )
    .unwrap();
    assert_eq!(artifacts.len(), 2);
    assert!(artifacts[0].verified_cached);
    assert_eq!(artifacts[1].installed_bytes, Some(5));
    let plan = estimate_space(
        &volumes(10, 13),
        &artifacts,
        &BTreeMap::from([("target".into(), 900)]),
        4,
    )
    .unwrap();
    assert!(plan.can_proceed);
    assert_eq!(plan.volumes[0].additional_peak_bytes, 10);
    assert_eq!(plan.volumes[1].additional_peak_bytes, 13);
    assert_eq!(plan.volumes[1].retained_existing_bytes, 900);
    let optional = pack_artifacts(
        &pack,
        &["mods/optional.jar".into()],
        &temp.0.join("target"),
        "cache",
        "target",
        &cache,
        &control,
    )
    .unwrap();
    assert!(
        !estimate_space(&volumes(10, 13), &optional, &BTreeMap::new(), 4)
            .unwrap()
            .can_proceed
    );
    assert!(
        pack_artifacts(
            &pack,
            &["mods/foreign.jar".into()],
            &temp.0.join("target"),
            "cache",
            "target",
            &cache,
            &control
        )
        .is_err()
    );
}
