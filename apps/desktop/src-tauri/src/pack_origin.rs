// SPDX-License-Identifier: MIT
//! Optional upstream coordinates retained from the trusted market download request.
//! Informational metadata; every future download must resolve and authorize again.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackProvider {
    Modrinth,
    Curseforge,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackOrigin {
    pub provider: PackProvider,
    pub project_id: String,
    pub version_id: String,
}
impl PackOrigin {
    pub(super) fn modrinth(project_id: &str, version_id: &str) -> Self {
        Self {
            provider: PackProvider::Modrinth,
            project_id: project_id.into(),
            version_id: version_id.into(),
        }
    }
    pub(super) fn curseforge(project_id: u32, file_id: u32) -> Self {
        Self {
            provider: PackProvider::Curseforge,
            project_id: project_id.to_string(),
            version_id: file_id.to_string(),
        }
    }
}
impl super::tasks::TaskRequest {
    pub(super) fn pack_origin(&self) -> Option<PackOrigin> {
        match self {
            Self::ModpackDownload {
                project_id,
                version_id,
                ..
            } => Some(PackOrigin::modrinth(project_id, version_id)),
            Self::CurseForgePackDownload {
                project_id,
                file_id,
                ..
            } => Some(PackOrigin::curseforge(*project_id, *file_id)),
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn old_games_and_local_archives_have_no_origin() {
        let game:super::super::Game=serde_json::from_value(serde_json::json!({"id":"legacy","name":"Local archive","version":"1.21.1","directory":"fixture","status":"installed","error":null,"pack_name":"A named pack","pack_version":"v1","pack_source_id":"123abc"})).unwrap();
        assert!(game.pack_origin.is_none());
        assert!(
            serde_json::to_value(game)
                .unwrap()
                .get("pack_origin")
                .is_none()
        );
        assert!(
            super::super::tasks::TaskRequest::GameInstall {
                game_id: "local".into(),
                initial_mod_plan: None,
                initial_environment_plan: None,
                initial_shared_configs: Vec::new(),
                initial_personal_options: None,
            }
            .pack_origin()
            .is_none()
        );
    }
    #[test]
    fn market_origin_survives_saved_task_reopen_and_game_persistence() {
        for request in [
            super::super::tasks::TaskRequest::ModpackDownload {
                archive_id: "archive".into(),
                project_id: "projectA".into(),
                version_id: "versionB".into(),
            },
            super::super::tasks::TaskRequest::CurseForgePackDownload {
                archive_id: "archive".into(),
                project_id: 123,
                file_id: 456,
            },
        ] {
            let expected = request.pack_origin().unwrap();
            let saved = serde_json::to_vec(&request).unwrap();
            let reopened: super::super::tasks::TaskRequest =
                serde_json::from_slice(&saved).unwrap();
            assert_eq!(reopened.pack_origin(), Some(expected.clone()));
            let game:super::super::Game=serde_json::from_value(serde_json::json!({"id":"market","name":"Display name unrelated to project","version":"1.21.1","directory":"fixture","status":"installed","error":null,"pack_version":"pack internal label","pack_origin":expected})).unwrap();
            let restored: super::super::Game =
                serde_json::from_slice(&serde_json::to_vec(&game).unwrap()).unwrap();
            assert_eq!(restored.pack_origin, Some(expected));
        }
    }
}
