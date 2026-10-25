// SPDX-License-Identifier: MIT
//! Independent file-format adapter. No PCL implementation code is reused.
use super::*;
use sha1::{Digest, Sha1};
const MAX_ROOTS: usize = 32;
const MAX_CANDIDATES: usize = 4096;

fn add(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if roots.len() >= MAX_ROOTS {
        return;
    }
    if let Ok(path) = real_directory(&path)
        && !roots.iter().any(|old| same(old, &path))
    {
        roots.push(path);
    }
}
fn same(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}
fn settings(root: &Path) -> Result<Vec<(String, String)>, Error> {
    let path = root.join("PCL/Setup.ini");
    if !path.try_exists()? {
        return Ok(Vec::new());
    }
    real_directory(&root.join("PCL"))?;
    let metadata = fs::symlink_metadata(&path)?;
    if linked(&metadata) || !metadata.is_file() || metadata.len() > 64 * 1024 {
        return Err("PCL 设置文件无效或超过 64 KiB".into());
    }
    let bytes = fs::read(path)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "PCL 设置编码暂不支持，请直接选择游戏目录")?;
    Ok(text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            matches!(key, "LaunchFolderSelect" | "LaunchFolders")
                .then(|| (key.to_owned(), value.trim().to_owned()))
        })
        .collect())
}
fn configured_path(root: &Path, value: &str) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 4096
        || value.contains(['\0', '\n', '\r'])
        || value.starts_with("\\\\")
        || value.starts_with("//")
    {
        return None;
    }
    let path = if let Some(relative) = value.strip_prefix('$') {
        let relative = relative.replace('\\', "/");
        let relative = relative.trim_start_matches('/');
        if relative
            .split('/')
            .any(|part| part == ".." || part.contains(':'))
        {
            return None;
        }
        root.join(relative)
    } else {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return None;
        }
        path
    };
    if path.to_string_lossy().starts_with("\\") || path.to_string_lossy().starts_with("//") {
        return None;
    }
    Some(path)
}

pub(super) fn inspect(root: &Path) -> Result<Vec<LauncherImportCandidate>, Error> {
    let root = real_directory(root)?;
    let mut roots = vec![root.clone()];
    let mut warnings = Vec::new();
    match settings(&root) {
        Ok(settings) => {
            for (key, value) in settings {
                let paths = if key == "LaunchFolders" {
                    value
                        .split('|')
                        .filter_map(|entry| entry.split_once('>').map(|(_, path)| path))
                        .collect::<Vec<_>>()
                } else {
                    vec![value.as_str()]
                };
                for path in paths.into_iter().take(MAX_ROOTS) {
                    if let Some(path) = configured_path(&root, path) {
                        add(&mut roots, path);
                    }
                }
            }
        }
        Err(error) => warnings.push(error.to_string()),
    }
    // Portable launchers also keep named game directories beside .minecraft.
    for entry in fs::read_dir(&root)?.take(512) {
        let entry = entry?;
        if entry.path().join("versions").is_dir() {
            add(&mut roots, entry.path());
        }
    }
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for path in roots {
        for mut candidate in inspect_direct(&path)? {
            let identity = candidate
                .runtime_source
                .as_ref()
                .map(|runtime| {
                    format!(
                        "{}:{}",
                        runtime.directory.display(),
                        runtime.client_version_id
                    )
                })
                .unwrap_or_else(|| format!("{}:{}", path.display(), candidate.id));
            let identity = if cfg!(windows) {
                identity.to_lowercase()
            } else {
                identity
            };
            if !seen.insert(identity.clone()) {
                continue;
            }
            let prefix = format!("{:x}", Sha1::digest(identity.as_bytes()));
            candidate.id = format!("{}-{}", &prefix[..12], candidate.id);
            candidate.warnings.extend(warnings.iter().cloned());
            output.push(candidate);
            if output.len() >= MAX_CANDIDATES {
                return Err("游戏数量超过 4096，请选择更具体的来源目录".into());
            }
        }
    }
    output.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(output)
}

/// Only the supplied host-owned roots and their immediate children are inspected.
/// Never follows shortcuts, executes launchers, or recursively scans a drive.
pub fn discover_launcher_imports(
    search_bases: &[PathBuf],
) -> Result<Vec<LauncherImportCandidate>, Error> {
    let mut roots = Vec::new();
    for base in search_bases.iter().take(8) {
        if let Ok(base) = real_directory(base) {
            if recognized(&base) {
                add(&mut roots, base.clone());
            }
            for entry in fs::read_dir(&base)?.take(256) {
                let entry = entry?;
                if recognized(&entry.path()) {
                    add(&mut roots, entry.path());
                }
            }
        }
    }
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        // An unreadable unrelated installation must not hide other discoveries.
        if let Ok(candidates) = inspect(&root) {
            for candidate in candidates {
                if seen.insert(candidate.id.clone()) {
                    output.push(candidate);
                }
                if output.len() >= MAX_CANDIDATES {
                    break;
                }
            }
        }
    }
    output.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(output)
}
fn recognized(path: &Path) -> bool {
    if !fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir() && !linked(&meta)) {
        return false;
    }
    path.join("versions").is_dir()
        || path.join(".minecraft/versions").is_dir()
        || path.join("mmc-pack.json").is_file()
        || path.join("PCL/Setup.ini").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cobble-pcl-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, path: &str, bytes: &[u8]) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        fn json(&self, path: &str, value: Value) {
            self.write(path, &serde_json::to_vec(&value).unwrap());
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn pcl_portable_discovery_reads_only_folder_keys_deduplicates_and_keeps_inheritance() {
        let f = Fixture::new();
        f.write("portable/PCL/Setup.ini",b"\xef\xbb\xbfLaunchFolderSelect:$.minecraft/\nLaunchFolders:Duplicate>$.minecraft/|Custom>$other/|UNC>\\\\server\\share\\\nLoginMsRefreshToken:never-import\n");
        f.json(
            "portable/.minecraft/versions/1.21.1/1.21.1.json",
            json!({"id":"1.21.1","mainClass":"net.minecraft.client.main.Main"}),
        );
        f.json("portable/.minecraft/versions/Fabric/Fabric.json",json!({"id":"Fabric","inheritsFrom":"1.21.1","libraries":[{"name":"net.fabricmc:fabric-loader:0.16.14"}],"mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient"}));
        f.json(
            "portable/other/versions/1.20.1/1.20.1.json",
            json!({"id":"1.20.1","mainClass":"net.minecraft.client.main.Main"}),
        );
        let candidates = discover_launcher_imports(std::slice::from_ref(&f.0)).unwrap();
        assert_eq!(candidates.len(), 3);
        let fabric = candidates.iter().find(|c| c.name == "Fabric").unwrap();
        assert_eq!(fabric.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(fabric.loader_kind.as_deref(), Some("fabric"));
        assert!(
            !serde_json::to_string(&candidates)
                .unwrap()
                .contains("never-import")
        );
        assert_eq!(fs::read(f.0.join("portable/PCL/Setup.ini")).unwrap(),b"\xef\xbb\xbfLaunchFolderSelect:$.minecraft/\nLaunchFolders:Duplicate>$.minecraft/|Custom>$other/|UNC>\\\\server\\share\\\nLoginMsRefreshToken:never-import\n");
        assert!(configured_path(&f.0, "$../outside/").is_none());
        assert!(configured_path(&f.0, "\\\\server\\share\\").is_none());
    }
    #[test]
    fn pcl_merged_neoforge_and_forge_use_exact_metadata_not_mod_names_or_instance_id() {
        let f = Fixture::new();
        for (name, kind, version, flag, mc) in [
            (
                "Mechanomania",
                "neoforge",
                "21.1.192",
                "--fml.neoForgeVersion",
                "1.21.1",
            ),
            (
                "Forge Pack",
                "forge",
                "47.4.28",
                "--fml.forgeVersion",
                "1.20.1",
            ),
        ] {
            f.json(&format!("versions/{name}/{name}.json"),json!({"id":name,"clientVersion":mc,"mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher","arguments":{"game":["--launchTarget","forgeclient",flag,version,"--fml.mcVersion",mc]}}));
            f.write(
                &format!("versions/{name}/mods/example.jar"),
                b"fixture-only",
            );
            let candidates = inspect(&f.0.join("versions").join(name)).unwrap();
            assert_eq!(candidates.len(), 1);
            let c = &candidates[0];
            assert!(c.unsupported_reason.is_none(), "{:?}", c.unsupported_reason);
            assert_eq!(c.loader_kind.as_deref(), Some(kind));
            assert_eq!(c.loader_version.as_deref(), Some(version));
            assert_eq!(c.minecraft_version.as_deref(), Some(mc));
            assert_eq!(c.content_roots[0].id, "isolated");
        }
        f.json(
            "versions/Incomplete/Incomplete.json",
            json!({"id":"1.21.1","mainClass":"net.minecraft.client.main.Main"}),
        );
        f.write(
            "versions/Incomplete/mods/neoforge-name-is-not-proof.jar",
            b"not-runtime-metadata",
        );
        assert!(
            inspect(&f.0)
                .unwrap()
                .iter()
                .find(|c| c.name == "Incomplete")
                .unwrap()
                .unsupported_reason
                .as_ref()
                .unwrap()
                .contains("未声明加载器")
        );
    }
}
