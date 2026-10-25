// SPDX-License-Identifier: MIT
//! Local automatic-update preferences. Exact manual selections and required dependencies
//! deliberately do not consult ignored versions; pins remain a separate hard constraint.
use super::*;
use cobble_content::{ModVersion, ModVersionDetails};

const FILE: &str = ".cobble-mod-update-policy.json";
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModUpdateChannel {
    #[default]
    Stable,
    Preview,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModProjectPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<ModUpdateChannel>,
    #[serde(default)]
    pub ignored_versions: BTreeSet<String>,
}

fn schema_version() -> u32 {
    1
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModUpdatePolicy {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub channel: ModUpdateChannel,
    /// Keys always include the provider, e.g. `modrinth:abc123`.
    #[serde(default)]
    pub projects: BTreeMap<String, ModProjectPolicy>,
}
impl Default for ModUpdatePolicy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            channel: ModUpdateChannel::Stable,
            projects: BTreeMap::new(),
        }
    }
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
impl ModUpdatePolicy {
    fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 || self.projects.len() > 1024 {
            return Err("Unsupported or oversized Mod update policy".into());
        }
        for (key, policy) in &self.projects {
            let Some((source, project)) = key.split_once(':') else {
                return Err("Mod policy project needs its source".into());
            };
            if !matches!(source, "modrinth" | "curseforge")
                || !identifier(project)
                || policy.ignored_versions.len() > 128
                || !policy.ignored_versions.iter().all(|id| identifier(id))
            {
                return Err("Invalid Mod policy project or ignored version".into());
            }
        }
        Ok(())
    }
    pub fn project(&self, source: &str, project: &str) -> Option<&ModProjectPolicy> {
        self.projects.get(&format!("{source}:{project}"))
    }
    pub fn channel_for(&self, source: &str, project: &str) -> ModUpdateChannel {
        self.project(source, project)
            .and_then(|p| p.channel)
            .unwrap_or(self.channel)
    }
}
pub(super) fn read(root: &Dir) -> Result<ModUpdatePolicy, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    match root.open_with(FILE, &options) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("Mod update policy exceeds its size limit".into());
            }
            let policy: ModUpdatePolicy = serde_json::from_slice(&bytes)?;
            policy.validate()?;
            Ok(policy)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ModUpdatePolicy::default()),
        Err(error) => Err(error.into()),
    }
}
pub fn read_mod_update_policy(directory: &Path) -> Result<ModUpdatePolicy, Error> {
    read(&open_root(directory)?)
}
/// The host must hold the same stopped-game write lease used for pin changes.
pub fn set_mod_update_policy(directory: &Path, policy: &ModUpdatePolicy) -> Result<(), Error> {
    policy.validate()?;
    let bytes = serde_json::to_vec_pretty(policy)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Mod update policy exceeds its size limit".into());
    }
    let root = open_root(directory)?;
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut temporary = root.open_with(format!("{FILE}.tmp"), &options)?;
    temporary.write_all(&bytes)?;
    temporary.write_all(b"\n")?;
    temporary.sync_all()?;
    drop(temporary);
    root.rename(format!("{FILE}.tmp"), &root, FILE)?;
    Ok(())
}
pub(super) fn select<'a>(
    policy: &ModUpdatePolicy,
    project: &str,
    current: &ModVersionDetails,
    versions: &'a [ModVersion],
) -> Option<&'a ModVersion> {
    let project_policy = policy.project("modrinth", project);
    versions
        .iter()
        .filter(|version| {
            version.version_id != current.version_id
                && version.date_published > current.date_published
                && (policy.channel_for("modrinth", project) == ModUpdateChannel::Preview
                    || version.version_type == "release")
                && !project_policy.is_some_and(|p| p.ignored_versions.contains(&version.version_id))
        })
        .max_by(|a, b| {
            a.date_published
                .cmp(&b.date_published)
                .then(a.version_id.cmp(&b.version_id))
        })
}
pub(super) fn validate_plan(root: &Dir, plan: &ModUpdatePlan) -> Result<(), Error> {
    if let Some(expected) = &plan.automatic_policy
        && &read(root)? != expected
    {
        return Err("Mod update preferences changed; preview the updates again".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn version(id: &str, kind: &str, date: &str) -> ModVersion {
        ModVersion {
            version_id: id.into(),
            name: id.into(),
            version_number: "not-semver".into(),
            version_type: kind.into(),
            date_published: date.into(),
        }
    }
    fn current() -> ModVersionDetails {
        ModVersionDetails {
            version_id: "old".into(),
            name: "old".into(),
            version_number: "old".into(),
            version_type: "release".into(),
            date_published: "2026-01-01".into(),
            changelog: String::new(),
        }
    }
    #[test]
    fn defaults_and_provider_metadata_preserve_stable_selection() {
        let policy: ModUpdatePolicy = serde_json::from_str("{}").unwrap();
        assert_eq!(policy, ModUpdatePolicy::default());
        let versions = [
            version("stable", "release", "2026-02-01"),
            version("beta", "beta", "2026-03-01"),
        ];
        assert_eq!(
            select(&policy, "project", &current(), &versions)
                .unwrap()
                .version_id,
            "stable"
        );
    }
    #[test]
    fn ignored_root_versions_can_be_restored_and_source_keys_do_not_collide() {
        let mut policy = ModUpdatePolicy::default();
        policy.projects.insert(
            "modrinth:project".into(),
            ModProjectPolicy {
                channel: Some(ModUpdateChannel::Preview),
                ignored_versions: BTreeSet::from(["beta".into()]),
            },
        );
        policy
            .projects
            .insert("curseforge:project".into(), ModProjectPolicy::default());
        let versions = [
            version("stable", "release", "2026-02-01"),
            version("beta", "beta", "2026-03-01"),
        ];
        assert_eq!(
            select(&policy, "project", &current(), &versions)
                .unwrap()
                .version_id,
            "stable"
        );
        policy
            .projects
            .get_mut("modrinth:project")
            .unwrap()
            .ignored_versions
            .clear();
        assert_eq!(
            select(&policy, "project", &current(), &versions)
                .unwrap()
                .version_id,
            "beta"
        );
        assert_eq!(
            policy.channel_for("curseforge", "project"),
            ModUpdateChannel::Stable
        );
        assert!(
            select(
                &policy,
                "project",
                &current(),
                &[version("earlier", "release", "2025-12-01")]
            )
            .is_none()
        );
    }
    #[test]
    fn rejects_unscoped_keys_unknown_schema_and_excessive_history() {
        let mut policy = ModUpdatePolicy::default();
        policy
            .projects
            .insert("project".into(), ModProjectPolicy::default());
        assert!(policy.validate().is_err());
        policy.projects.clear();
        policy.schema_version = 2;
        assert!(policy.validate().is_err());
        policy.schema_version = 1;
        policy.projects.insert(
            "modrinth:project".into(),
            ModProjectPolicy {
                channel: None,
                ignored_versions: (0..129).map(|n| format!("v{n}")).collect(),
            },
        );
        assert!(policy.validate().is_err());
    }
}
