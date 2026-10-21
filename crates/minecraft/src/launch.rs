// SPDX-License-Identifier: MIT
//! Modern Mojang argument arrays, evaluated without shell tokenization.
use crate::metadata::{Rule, allowed};
use crate::{MetadataError, PlatformContext};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(untagged)]
enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: Values },
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Values {
    One(String),
    Many(Vec<String>),
}

/// Resolve known local placeholders, retaining only named account placeholders
/// for the future authenticated spawn step. Each JSON string stays one argv item.
pub fn resolve_arguments(
    values: &serde_json::Value,
    platform: &PlatformContext,
    placeholders: &HashMap<String, String>,
) -> Result<Vec<String>, MetadataError> {
    let arguments: Vec<Argument> =
        serde_json::from_value(values.clone()).map_err(|_| MetadataError::InvalidMetadata)?;
    let mut output = Vec::new();
    for argument in arguments {
        let values = match argument {
            Argument::Plain(value) => vec![value],
            Argument::Conditional { rules, value } => {
                if !allowed(Some(&rules), platform)? {
                    continue;
                }
                match value {
                    Values::One(v) => vec![v],
                    Values::Many(v) => v,
                }
            }
        };
        for value in values {
            output.push(substitute(&value, placeholders)?);
        }
    }
    Ok(output)
}

pub fn substitute(
    value: &str,
    placeholders: &HashMap<String, String>,
) -> Result<String, MetadataError> {
    let mut remaining = value;
    let mut output = String::new();
    while let Some(start) = remaining.find("${") {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 2..];
        let end = after.find('}').ok_or(MetadataError::InvalidMetadata)?;
        let name = &after[..end];
        if let Some(replacement) = placeholders.get(name) {
            output.push_str(replacement);
        } else if matches!(
            name,
            "auth_player_name"
                | "auth_uuid"
                | "auth_access_token"
                | "auth_session"
                | "user_properties"
                | "clientid"
                | "auth_xuid"
                | "user_type"
        ) {
            output.push_str(&remaining[start..start + end + 3]);
        } else {
            return Err(MetadataError::Unsupported("argument placeholder"));
        }
        remaining = &after[end + 1..];
    }
    output.push_str(remaining);
    Ok(output)
}

/// Modern LWJGL natives are ordinary library artifacts; the metadata may list
/// every CPU for an OS. Include only the current JVM architecture in the plan.
pub fn native_artifact_matches(path: &str, platform: &PlatformContext) -> bool {
    if !path.contains("-natives-") {
        return true;
    }
    let native_arch = if path.ends_with("-arm64.jar") {
        "aarch64"
    } else if path.ends_with("-x86.jar") {
        "x86"
    } else {
        "amd64"
    };
    native_arch == platform.arch || (native_arch == "amd64" && platform.arch == "x86_64")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn arrays_rules_spaces_and_account_templates() {
        let platform = PlatformContext {
            os: "windows".into(),
            arch: "amd64".into(),
            os_version: None,
            features: HashMap::new(),
        };
        let vars = HashMap::from([("game_directory".into(), "C:/Game folder".into())]);
        let result = resolve_arguments(&json!([
            "--gameDir", "${game_directory}", "${auth_access_token}",
            {"rules":[{"action":"allow","os":{"name":"osx"}}],"value":"mac"},
            {"rules":[{"action":"allow","features":{"optional":false}}],"value":["two words", "last"]}
        ]), &platform, &vars).unwrap();
        assert_eq!(
            result,
            [
                "--gameDir",
                "C:/Game folder",
                "${auth_access_token}",
                "two words",
                "last"
            ]
        );
        assert!(substitute("${unknown}", &vars).is_err());
        assert_eq!(
            substitute("${auth_session}/${user_properties}", &vars).unwrap(),
            "${auth_session}/${user_properties}"
        );
        assert!(native_artifact_matches(
            "lwjgl-natives-windows.jar",
            &platform
        ));
        assert!(!native_artifact_matches(
            "lwjgl-natives-windows-arm64.jar",
            &platform
        ));
        assert!(!native_artifact_matches(
            "lwjgl-natives-windows-x86.jar",
            &platform
        ));
    }
}
