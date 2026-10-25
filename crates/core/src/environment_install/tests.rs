// SPDX-License-Identifier: MIT
use super::*;
use cobble_content::{
    EnvironmentPlan, EnvironmentSelection, EnvironmentSource, ModrinthFile, ModrinthPlan,
    ResolvedMod,
};
use sha1::{Digest, Sha1};
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cobble-environment-{}-{}",
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
fn graph(project: &str, name: &str, bytes: &[u8]) -> ModrinthPlan {
    ModrinthPlan {
        project_id: project.into(),
        version_id: "v1".into(),
        mods: vec![ResolvedMod {
            project_id: project.into(),
            version_id: "v1".into(),
            name: project.into(),
            version_number: "one".into(),
            required_by: None,
            file: ModrinthFile {
                url: if project == "mr" {
                    format!("https://cdn.modrinth.com/data/mr/versions/v1/{name}")
                } else {
                    format!("https://edge.forgecdn.net/files/1/1/{name}")
                },
                filename: name.into(),
                size: bytes.len() as u64,
                sha1: format!("{:x}", Sha1::digest(bytes)),
                sha512: None,
            },
        }],
    }
}
fn plan() -> EnvironmentPlan {
    EnvironmentPlan {
        game_version: "1.21.1".into(),
        loader: "fabric".into(),
        roots: vec![
            EnvironmentSelection {
                source: EnvironmentSource::Modrinth,
                project_id: "mr".into(),
                version_id: "v1".into(),
            },
            EnvironmentSelection {
                source: EnvironmentSource::Curseforge,
                project_id: "2".into(),
                version_id: "v1".into(),
            },
        ],
        modrinth: Some(graph("mr", "mr.jar", b"MR fixture")),
        curseforge: Some(cobble_content::curseforge::InstallPlan {
            graph: graph("2", "cf.jar", b"CF fixture"),
            restricted: vec![],
            incompatible: vec![],
        }),
        dependency_edges: vec![],
    }
}
#[test]
fn mixed_environment_rejects_cross_source_file_and_content_collisions() {
    let mut value = plan();
    validate_environment_plan(&value).unwrap();
    value.curseforge.as_mut().unwrap().graph.mods[0]
        .file
        .filename = "MR.jar".into();
    assert!(validate_environment_plan(&value).is_err());
    let mut value = plan();
    value.curseforge.as_mut().unwrap().graph.mods[0].file.sha1 =
        value.modrinth.as_ref().unwrap().mods[0].file.sha1.clone();
    assert!(validate_environment_plan(&value).is_err());
}
#[tokio::test]
async fn mixed_environment_partial_source_failure_stays_blocked_and_exact_retry_finishes() {
    let temp = Temp::new();
    std::fs::create_dir(temp.0.join("mods")).unwrap();
    std::fs::write(temp.0.join("mods/mr.jar"), b"MR fixture").unwrap();
    std::fs::write(temp.0.join("mods/cf.jar"), b"CF fixture").unwrap();
    let control = crate::OperationControl::new(std::sync::Arc::new(|_| {}));
    assert!(
        install_environment_plan_controlled(&plan(), &temp.0, None, control.clone())
            .await
            .is_err()
    );
    assert!(ensure_environment_complete(&temp.0).is_err());
    assert!(read_environment_plan(&temp.0).unwrap().is_some());
    let result =
        install_environment_plan_controlled(&plan(), &temp.0, Some("fixture-key".into()), control)
            .await
            .unwrap();
    assert_eq!(result.reused, 2);
    ensure_environment_complete(&temp.0).unwrap();
    assert_eq!(crate::list_managed_modrinth_mods(&temp.0).unwrap().len(), 1);
    assert_eq!(
        crate::list_managed_curseforge_mods(&temp.0).unwrap().len(),
        1
    );
    let mut changed = plan();
    changed.roots.pop();
    assert!(
        install_environment_plan_controlled(
            &changed,
            &temp.0,
            Some("fixture-key".into()),
            crate::OperationControl::new(std::sync::Arc::new(|_| {}))
        )
        .await
        .is_err()
    );
}
#[tokio::test]
async fn mixed_environment_precancelled_never_publishes_pending_or_mods() {
    let temp = Temp::new();
    let control = crate::OperationControl::new(std::sync::Arc::new(|_| {}));
    control.cancel();
    assert!(
        install_environment_plan_controlled(&plan(), &temp.0, None, control)
            .await
            .is_err()
    );
    assert!(!temp.0.join(MARKER).exists());
    assert!(!temp.0.join("mods").exists());
}
#[test]
fn mixed_environment_manual_handle_must_match_exact_requested_artifact() {
    let temp = Temp::new();
    let mut value = plan();
    let cf = value.curseforge.as_mut().unwrap();
    cf.graph.mods[0].file.url.clear();
    cf.restricted
        .push(cobble_content::curseforge::RestrictedFile {
            project_id: "2".into(),
            file_id: "v1".into(),
            name: "fixture".into(),
            file_name: "cf.jar".into(),
            size_bytes: 10,
            website_url: None,
            reason: "manual".into(),
        });
    let input = temp.0.join("selected.jar");
    std::fs::write(&input, b"wrong file").unwrap();
    assert!(
        import_environment_manual_file(&value, "2", "v1", std::fs::File::open(&input).unwrap())
            .is_err()
    );
    assert!(
        import_environment_manual_file(&value, "2", "other", std::fs::File::open(&input).unwrap())
            .is_err()
    );
}
