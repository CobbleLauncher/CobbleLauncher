// SPDX-License-Identifier: MIT
use super::*;
use crate::{
    BoxError,
    curseforge::{CurseForgeClient, solver as cf},
    modrinth::{self, Api, reverse::CachedApi, solver as mr},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

fn error(text: &str) -> BoxError {
    std::io::Error::other(text).into()
}
fn validate(
    roots: &[EnvironmentRoot],
    preferred: &[String],
    loaders: &[String],
) -> Result<Vec<String>, BoxError> {
    if roots.is_empty() || roots.len() > 128 || preferred.len() > 32 || loaders.len() > 3 {
        return Err(error("Select1to128projects and at most32game versions"));
    }
    let mut identities = BTreeSet::new();
    for root in roots {
        if !identities.insert((root.source, root.project_id.clone())) {
            return Err(error("Duplicate environment project"));
        }
        match root.source {
            EnvironmentSource::Modrinth => {
                modrinth::identifier(&root.project_id)?;
                if let Some(v) = &root.version_id {
                    modrinth::identifier(v)?;
                }
            }
            EnvironmentSource::Curseforge => {
                cf::parse_id(&root.project_id)?;
                if let Some(v) = &root.version_id {
                    cf::parse_id(v)?;
                }
            }
        }
    }
    let loaders = if loaders.is_empty() {
        vec!["fabric".into(), "forge".into(), "neoforge".into()]
    } else {
        loaders.to_vec()
    };
    for loader in &loaders {
        modrinth::target("1.21.1", loader)?;
    }
    for mc in preferred {
        modrinth::target(mc, &loaders[0])?;
    }
    Ok(loaders)
}

pub async fn reverse_configure_environment(
    roots: &[EnvironmentRoot],
    preferred_game_versions: &[String],
    loaders: &[String],
    curseforge: Option<&CurseForgeClient>,
) -> Result<ReverseEnvironmentConfiguration, BoxError> {
    let loaders = validate(roots, preferred_game_versions, loaders)?;
    if roots
        .iter()
        .any(|r| r.source == EnvironmentSource::Curseforge)
        && curseforge.is_none()
    {
        return Err(crate::curseforge::ApiError::MissingCredential.into());
    }
    let started = Instant::now();
    let mut live = modrinth::LiveApi::new()?;
    let mut mr_api = CachedApi::new(&mut live);
    let mut cf_api = curseforge.map(cf::Cache::new);
    let unpinned_mr: Vec<_> = roots
        .iter()
        .filter(|r| r.source == EnvironmentSource::Modrinth && r.version_id.is_none())
        .map(|r| r.project_id.clone())
        .collect();
    mr_api.prefetch_projects(&unpinned_mr).await?;
    let mut canonical = Vec::new();
    let mut common: Option<BTreeSet<String>> = None;
    let mut discovery_truncated = false;
    for root in roots {
        let mut root = root.clone();
        let advertised = match root.source {
            EnvironmentSource::Modrinth => {
                let project = mr_api.project(&root.project_id).await?;
                modrinth::supported_client_project(&project)?;
                root.project_id = project.id;
                let versions = if let Some(pin) = &root.version_id {
                    let exact = mr_api.version(pin).await?;
                    if exact.project_id != root.project_id || exact.id != *pin {
                        return Err(error(
                            "Selected Modrinth version belongs to another project",
                        ));
                    }
                    vec![exact]
                } else {
                    mr_api.all_versions(&root.project_id).await?
                };
                versions
                    .iter()
                    .filter(|v| {
                        v.project_id == root.project_id
                            && (root.version_id.is_some() || v.version_type == "release")
                    })
                    .flat_map(|v| v.game_versions.iter())
                    .filter(|v| numeric(v).is_some())
                    .cloned()
                    .collect()
            }
            EnvironmentSource::Curseforge => {
                let (versions, truncated) = cf_api
                    .as_mut()
                    .ok_or(crate::curseforge::ApiError::MissingCredential)?
                    .root_targets(&root)
                    .await?;
                discovery_truncated |= truncated;
                versions
            }
        };
        common = Some(match common {
            None => advertised,
            Some(previous) => previous.intersection(&advertised).cloned().collect(),
        });
        if canonical
            .iter()
            .any(|r: &EnvironmentRoot| r.source == root.source && r.project_id == root.project_id)
        {
            return Err(error("Duplicate canonical environment project"));
        }
        canonical.push(root);
    }
    let mut versions = if preferred_game_versions.is_empty() {
        common.unwrap_or_default().into_iter().collect::<Vec<_>>()
    } else {
        preferred_game_versions.to_vec()
    };
    if preferred_game_versions.is_empty() {
        versions.sort_by_key(|v| std::cmp::Reverse(numeric(v)));
    }
    let mut response = ReverseEnvironmentConfiguration {
        choices: vec![],
        conflicts: vec![],
        stats: crate::ReverseConfigurationStats::default(),
        limitations: vec![
            "client_metadata_only".into(),
            "stable_unpinned_versions_only".into(),
            "runtime_mod_ids_unknown".into(),
            "optional_embedded_include_not_added".into(),
        ],
    };
    if discovery_truncated {
        response
            .limitations
            .push("curseforge_discovery_first_200_files".into());
    }
    let mut seen = BTreeSet::new();
    for mc in versions {
        for loader in &loaders {
            if !seen.insert((mc.clone(), loader.clone())) {
                continue;
            }
            if response.stats.targets_considered >= 32 {
                if !response.limitations.iter().any(|s| s == "target_limit_32") {
                    response.limitations.push("target_limit_32".into());
                }
                break;
            }
            response.stats.targets_considered += 1;
            let attempt = collect(&mut mr_api, cf_api.as_mut(), &canonical, &mc, loader).await;
            match attempt {
                Ok(catalog) => {
                    response.stats.candidate_versions += catalog.request.candidates.len();
                    if catalog.truncated
                        && !response
                            .limitations
                            .iter()
                            .any(|s| s == "candidate_limit_32_per_project")
                    {
                        response
                            .limitations
                            .push("candidate_limit_32_per_project".into());
                    }
                    let mut solve_elapsed = 0;
                    let solved = super::catalog::solve_catalog_measured(
                        catalog,
                        &canonical,
                        &mc,
                        loader,
                        &mut solve_elapsed,
                    )
                    .await;
                    response.stats.solve_elapsed_ms += solve_elapsed;
                    match solved {
                        Ok((plan, elapsed, _)) => {
                            let _ = elapsed;
                            response.choices.push(ReverseEnvironmentChoice {
                                game_version: mc.clone(),
                                loader: loader.clone(),
                                roots: plan.roots.clone(),
                                plan,
                            });
                        }
                        Err(e) => response
                            .conflicts
                            .push(super::catalog::conflict(&e, &mc, loader, &canonical)),
                    }
                }
                Err(e) => response
                    .conflicts
                    .push(super::catalog::conflict(&e, &mc, loader, &canonical)),
            }
        }
    }
    if response.choices.is_empty() && response.conflicts.is_empty() {
        response.conflicts.push(EnvironmentConflict{game_version:String::new(),loader:String::new(),code:if discovery_truncated{"candidate_limit".into()}else{"no_common_target".into()},message:"Selected projects advertise no common stable game target within the available metadata".into(),projects:canonical.iter().map(|r|EnvironmentProject{source:r.source,project_id:r.project_id.clone()}).collect()});
    }
    response.stats.metadata_requests =
        mr_api.requests() + cf_api.as_ref().map_or(0, |c| c.requests);
    response.stats.metadata_elapsed_ms =
        (started.elapsed().as_millis() as u64).saturating_sub(response.stats.solve_elapsed_ms);
    Ok(response)
}
fn numeric(value: &str) -> Option<Vec<u32>> {
    value
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

async fn collect<A: Api, B: cf::Api>(
    mr_api: &mut A,
    cf_api: Option<&mut cf::Cache<B>>,
    roots: &[EnvironmentRoot],
    mc: &str,
    loader: &str,
) -> Result<super::catalog::Catalog, BoxError> {
    let mut catalog = super::catalog::Catalog::default();
    let mr_roots: Vec<_> = roots
        .iter()
        .filter(|r| r.source == EnvironmentSource::Modrinth)
        .cloned()
        .collect();
    if !mr_roots.is_empty() {
        let projects = mr_roots
            .iter()
            .map(|r| r.project_id.clone())
            .collect::<Vec<_>>();
        let pins: BTreeMap<_, _> = mr_roots
            .iter()
            .filter_map(|r| {
                r.version_id
                    .as_ref()
                    .map(|v| (r.project_id.clone(), v.clone()))
            })
            .collect();
        let mut stats = mr::AttemptStats::default();
        let prepared =
            mr::collect_candidates(mr_api, &projects, &pins, mc, loader, &mut stats).await?;
        catalog.append(prepared.into_environment_catalog(stats.truncated));
    }
    let cf_roots: Vec<_> = roots
        .iter()
        .filter(|r| r.source == EnvironmentSource::Curseforge)
        .cloned()
        .collect();
    if !cf_roots.is_empty() {
        let cf_api = cf_api.ok_or(crate::curseforge::ApiError::MissingCredential)?;
        cf_api.validate_mod_roots(&cf_roots).await?;
        let mut cf_catalog = cf::collect(cf_api, &cf_roots, mc, Some(loader)).await?;
        for candidate in &mut cf_catalog.request.candidates {
            if cf_catalog
                .artifacts
                .get(&(candidate.package.clone(), candidate.version.clone()))
                .is_some_and(|a| a.destination != crate::curseforge::PackFileDestination::Mods)
            {
                candidate.unavailable =
                    Some("Environment Mod selection cannot install a non-Mod dependency".into());
            }
        }
        catalog.append(cf_catalog);
    }
    Ok(catalog)
}

pub async fn resolve_environment_selected(
    roots: &[EnvironmentSelection],
    mc: &str,
    loader: &str,
    curseforge: Option<&CurseForgeClient>,
) -> Result<EnvironmentPlan, BoxError> {
    let roots: Vec<_> = roots
        .iter()
        .map(|r| EnvironmentRoot {
            source: r.source,
            project_id: r.project_id.clone(),
            version_id: Some(r.version_id.clone()),
        })
        .collect();
    validate(&roots, &[mc.into()], &[loader.into()])?;
    let mut live = modrinth::LiveApi::new()?;
    let mut mr_api = CachedApi::new(&mut live);
    let mut cf_api = curseforge.map(cf::Cache::new);
    let catalog = collect(&mut mr_api, cf_api.as_mut(), &roots, mc, loader).await?;
    Ok(super::catalog::solve_catalog(catalog, &roots, mc, loader)
        .await?
        .0)
}
