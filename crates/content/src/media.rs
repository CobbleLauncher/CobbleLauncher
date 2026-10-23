// SPDX-License-Identifier: MIT
//! Native provider artwork URL policy; metadata never authorizes arbitrary hosts.
use url::Url;
pub fn valid_image_url(provider: &str, value: &str) -> bool {
    if value.len() > 2048 {
        return false;
    }
    Url::parse(value).is_ok_and(|url| {
        let source = match provider {
            "modrinth" => {
                url.host_str() == Some("cdn.modrinth.com") && url.path().starts_with("/data/")
            }
            "curseforge" => {
                matches!(
                    url.host_str(),
                    Some("media.forgecdn.net" | "mediafilez.forgecdn.net")
                ) && (url.path().starts_with("/avatars/")
                    || url.path().starts_with("/attachments/"))
            }
            _ => false,
        };
        source
            && url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && !value.chars().any(|c| c.is_control() || c.is_whitespace())
    })
}
pub(crate) fn optional(provider: &str, value: Option<String>) -> Option<String> {
    value.filter(|url| valid_image_url(provider, url))
}
pub(crate) fn gallery(provider: &str, values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values.into_iter().take(64) {
        if valid_image_url(provider, &value) && !result.contains(&value) {
            result.push(value);
            if result.len() == 12 {
                break;
            }
        }
    }
    result
}
pub(crate) fn modrinth_image<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    use serde::Deserialize;
    Ok(optional(
        "modrinth",
        Option::<String>::deserialize(deserializer)?,
    ))
}
pub(crate) fn modrinth_gallery<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    use serde::Deserialize;
    Ok(gallery(
        "modrinth",
        Vec::<String>::deserialize(deserializer)?,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_public_provider_artwork_paths_are_allowed() {
        assert!(valid_image_url(
            "modrinth",
            "https://cdn.modrinth.com/data/Project/icon.png"
        ));
        assert!(valid_image_url(
            "curseforge",
            "https://media.forgecdn.net/attachments/1/2/screen.png"
        ));
        for url in [
            "https://cdn.modrinth.com.evil.example/data/a.png",
            "https://cdn.modrinth.com/data/a.svg?token=secret",
            "https://user@cdn.modrinth.com/data/a.png",
            "https://127.0.0.1/data/a.png",
            "https://media.forgecdn.net/files/1/mod.jar",
        ] {
            assert!(!valid_image_url("modrinth", url));
            assert!(!valid_image_url("curseforge", url));
        }
    }
}

#[cfg(test)]
#[test]
fn search_metadata_retains_artwork_without_detail_requests() {
    let page:crate::SearchResults=serde_json::from_value(serde_json::json!({"hits":[{"project_id":"Project","slug":"project","title":"Project","description":"Fixture","author":"Author","downloads":1,"icon_url":"https://cdn.modrinth.com/data/Project/icon.png","featured_gallery":"https://cdn.modrinth.com/data/Project/images/preview.webp","gallery":["https://cdn.modrinth.com/data/Project/images/preview.webp","https://evil.example/tracker.png"]}],"offset":0,"total_hits":1})).unwrap();
    assert!(page.hits[0].icon_url.is_some());
    assert!(page.hits[0].preview_url.is_some());
    assert_eq!(page.hits[0].gallery.len(), 1);
    let plain:crate::SearchResults=serde_json::from_value(serde_json::json!({"hits":[{"project_id":"Old","slug":"old","title":"Old","description":"Fixture","author":"Author","downloads":1}],"offset":0,"total_hits":1})).unwrap();
    assert!(plain.hits[0].icon_url.is_none() && plain.hits[0].gallery.is_empty());
}
