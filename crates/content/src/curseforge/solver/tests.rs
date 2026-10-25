// SPDX-License-Identifier: MIT
use super::*;
use serde_json::json;

pub(crate) struct Fixture {
    pub files: Vec<RawFile>,
    pub restricted: bool,
}
impl Api for Fixture {
    async fn project(&mut self, id: u32) -> Result<Project, ApiError> {
        Ok(Project {
            icon_url: None,
            preview_url: None,
            gallery: vec![],
            class_id: 6,
            project_id: id.to_string(),
            slug: format!("p{id}"),
            title: format!("Project{id}"),
            description: String::new(),
            author: String::new(),
            downloads: 0,
            website_url: Some(format!(
                "https://www.curseforge.com/minecraft/mc-mods/p{id}"
            )),
            allow_distribution: Some(!self.restricted),
            available: true,
        })
    }
    async fn exact(&mut self, p: u32, id: u32) -> Result<RawFile, ApiError> {
        self.files
            .iter()
            .find(|f| f.mod_id == p && f.id == id)
            .cloned()
            .ok_or(ApiError::NotFound)
    }
    async fn page(
        &mut self,
        p: u32,
        mc: Option<&str>,
        loader: Option<&str>,
        _: u32,
    ) -> Result<(Vec<RawFile>, u64), ApiError> {
        let files: Vec<_> = self
            .files
            .iter()
            .filter(|f| f.mod_id == p && mc.is_none_or(|mc| compatible(f, mc, loader)))
            .cloned()
            .collect();
        let total = files.len() as u64;
        Ok((files, total))
    }
}
pub(crate) fn file(project: u32, id: u32, date: &str, deps: serde_json::Value) -> RawFile {
    serde_json::from_value(json!({"id":id,"gameId":432,"modId":project,"isAvailable":true,"displayName":format!("opaque-{id}"),"fileName":format!("p{project}-{id}.jar"),"releaseType":1,"fileDate":date,"fileLength":42,"downloadUrl":format!("https://edge.forgecdn.net/files/1/p{project}-{id}.jar"),"gameVersions":["1.21.1","Fabric"],"dependencies":deps,"hashes":[{"algo":1,"value":format!("{id:040x}")}]})).unwrap()
}
fn root(project: u32, file: Option<u32>) -> EnvironmentRoot {
    EnvironmentRoot {
        source: EnvironmentSource::Curseforge,
        project_id: project.to_string(),
        version_id: file.map(|v| v.to_string()),
    }
}

#[test]
fn cf_backtracks_files_preserves_exact_root_and_restricted_routes() {
    let fixture = Fixture {
        restricted: true,
        files: vec![
            file(
                1,
                10,
                "2026-03-01T00:00:00Z",
                json!([{"modId":2,"relationType":3},{"modId":3,"relationType":3}]),
            ),
            file(
                2,
                21,
                "2026-03-01T00:00:00Z",
                json!([{"modId":3,"relationType":5}]),
            ),
            file(2, 20, "2026-02-01T00:00:00Z", json!([])),
            file(3, 30, "2026-03-01T00:00:00Z", json!([])),
        ],
    };
    let roots = vec![root(1, Some(10))];
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let mut cache = Cache::new(fixture);
            let catalog = collect(&mut cache, &roots, "1.21.1", Some("fabric"))
                .await
                .unwrap();
            let (plan, _, count) =
                crate::environment::catalog::solve_catalog(catalog, &roots, "1.21.1", "fabric")
                    .await
                    .unwrap();
            assert_eq!(count, 4);
            let cf = plan.curseforge.unwrap();
            assert!(
                cf.graph
                    .mods
                    .iter()
                    .any(|m| m.project_id == "2" && m.version_id == "20")
            );
            assert_eq!(cf.restricted.len(), 3);
            assert!(cf.graph.mods.iter().all(|m| m.file.url.is_empty()));
            assert_eq!(plan.dependency_edges.len(), 2);
        });
}

#[test]
fn embedded_optional_and_include_do_not_fabricate_exact_dependencies() {
    let fixture = Fixture {
        restricted: false,
        files: vec![file(
            1,
            10,
            "2026-01-01T00:00:00Z",
            json!([
                {"modId":2,"relationType":1},{"modId":3,"relationType":2},{"modId":4,"relationType":6}
            ]),
        )],
    };
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let mut cache = Cache::new(fixture);
            let catalog = collect(&mut cache, &[root(1, Some(10))], "1.21.1", Some("fabric"))
                .await
                .unwrap();
            assert_eq!(catalog.request.candidates.len(), 1);
            assert!(catalog.request.candidates[0].required.is_empty());
            assert_eq!(cache.requests, 2);
        });
}

#[test]
fn exact_cross_root_pin_incompatibility_is_structured() {
    let fixture = Fixture {
        restricted: false,
        files: vec![
            file(
                1,
                10,
                "2026-01-01T00:00:00Z",
                json!([{"modId":2,"relationType":5}]),
            ),
            file(2, 20, "2026-01-01T00:00:00Z", json!([])),
        ],
    };
    let roots = vec![root(1, Some(10)), root(2, Some(20))];
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let catalog = collect(&mut Cache::new(fixture), &roots, "1.21.1", Some("fabric"))
                .await
                .unwrap();
            let err =
                crate::environment::catalog::solve_catalog(catalog, &roots, "1.21.1", "fabric")
                    .await
                    .err()
                    .unwrap();
            assert_eq!(
                err.downcast_ref::<crate::dependency_solver::SolverConflict>()
                    .unwrap()
                    .code,
                "unsatisfiable"
            );
        });
}
