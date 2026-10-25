// SPDX-License-Identifier: MIT
use super::Error;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Package {
    pub major: u32,
    pub release: String,
    pub url: String,
    pub size: u64,
    pub sha256: [u8; 32],
}
#[derive(Deserialize)]
struct Release {
    vendor: String,
    release_type: String,
    release_name: String,
    version_data: Version,
    binaries: Vec<Binary>,
}
#[derive(Deserialize)]
struct Version {
    major: u32,
}
#[derive(Deserialize)]
struct Binary {
    architecture: String,
    os: String,
    image_type: String,
    jvm_impl: String,
    heap_size: String,
    package: Artifact,
}
#[derive(Deserialize)]
struct Artifact {
    name: String,
    link: String,
    size: u64,
    checksum: String,
}
pub(super) fn metadata_url(major: u32) -> Result<String, Error> {
    if !(8..=99).contains(&major) {
        return Err("JAVA_UNSUPPORTED_MAJOR".into());
    }
    Ok(format!(
        "https://api.adoptium.net/v3/assets/feature_releases/{major}/ga?architecture=x64&heap_size=normal&image_type=jre&jvm_impl=hotspot&os=windows&page=0&page_size=1&project=jdk&sort_order=DESC&vendor=eclipse"
    ))
}
pub(super) fn parse(bytes: &[u8], required: u32) -> Result<Package, Error> {
    let releases: Vec<Release> = serde_json::from_slice(bytes)?;
    let [release] = releases.as_slice() else {
        return Err("JAVA_NO_OFFICIAL_RUNTIME".into());
    };
    if release.vendor != "eclipse"
        || release.release_type != "ga"
        || release.version_data.major != required
        || release.release_name.len() > 128
        || !release
            .release_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._+".contains(&b))
    {
        return Err("JAVA_METADATA_MISMATCH".into());
    }
    let matching = release
        .binaries
        .iter()
        .filter(|binary| {
            binary.os == "windows"
                && binary.architecture == "x64"
                && binary.image_type == "jre"
                && binary.jvm_impl == "hotspot"
                && binary.heap_size == "normal"
        })
        .collect::<Vec<_>>();
    let [binary] = matching.as_slice() else {
        return Err("JAVA_NO_OFFICIAL_RUNTIME".into());
    };
    let artifact = &binary.package;
    let prefix =
        format!("https://github.com/adoptium/temurin{required}-binaries/releases/download/");
    let relative = artifact
        .link
        .strip_prefix(&prefix)
        .ok_or("JAVA_UNTRUSTED_SOURCE")?;
    let (tag, filename) = relative.split_once('/').ok_or("JAVA_UNTRUSTED_SOURCE")?;
    if tag.replace("%2B", "+").replace("%2b", "+") != release.release_name
        || filename != artifact.name
        || artifact.name.contains('/')
        || !artifact.name.ends_with(".zip")
        || !artifact
            .name
            .starts_with(&format!("OpenJDK{required}U-jre_x64_windows_hotspot_"))
        || artifact.size == 0
        || artifact.size > 512 * 1024 * 1024
        || artifact.checksum.len() != 64
        || !artifact.checksum.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("JAVA_METADATA_MISMATCH".into());
    }
    cobble_store::SafeRelativePath::parse(&artifact.name)?;
    let mut sha256 = [0; 32];
    for (output, pair) in sha256
        .iter_mut()
        .zip(artifact.checksum.as_bytes().as_chunks::<2>().0)
    {
        *output = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(Package {
        major: required,
        release: release.release_name.clone(),
        url: artifact.link.clone(),
        size: artifact.size,
        sha256,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> serde_json::Value {
        serde_json::json!([{"vendor":"eclipse","release_type":"ga","release_name":"jdk-21.0.1+1","version_data":{"major":21},"binaries":[{"architecture":"x64","os":"windows","image_type":"jre","jvm_impl":"hotspot","heap_size":"normal","package":{"name":"OpenJDK21U-jre_x64_windows_hotspot_21.0.1_1.zip","link":"https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.1%2B1/OpenJDK21U-jre_x64_windows_hotspot_21.0.1_1.zip","size":123,"checksum":"ab".repeat(32)}}]}])
    }
    #[test]
    fn official_ga_metadata_is_exactly_bound_to_major_platform_archive_and_hash() {
        assert_eq!(
            parse(&serde_json::to_vec(&fixture()).unwrap(), 21)
                .unwrap()
                .sha256,
            [0xab; 32]
        );
        for (field, value) in [
            ("architecture", "aarch64"),
            ("os", "linux"),
            ("image_type", "jdk"),
        ] {
            let mut bad = fixture();
            bad[0]["binaries"][0][field] = value.into();
            assert!(parse(&serde_json::to_vec(&bad).unwrap(), 21).is_err());
        }
        let mut bad = fixture();
        bad[0]["binaries"][0]["package"]["link"] =
            "https://github.com/untrusted/runtime.zip".into();
        assert!(parse(&serde_json::to_vec(&bad).unwrap(), 21).is_err());
        assert!(parse(b"[]", 21).is_err());
        assert!(metadata_url(0).is_err());
    }
}
