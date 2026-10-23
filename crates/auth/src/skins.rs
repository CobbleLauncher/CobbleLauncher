// SPDX-License-Identifier: MIT
//! Account appearance. Credentialed calls stay on the fixed Minecraft origin;
//! only public texture bytes pass through the shared metadata cache.
use crate::protocol::{Endpoint, Http, Transport};
use crate::{BoxError, get_player_profile, launch_identity};
use base64::{Engine, engine::general_purpose::STANDARD};
use curl::easy::{Easy, Form, List, NetRc};
use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Cursor, Read},
    path::PathBuf,
    time::Duration,
};
static SKIN_OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinPreview {
    pub image: String,
    pub avatar: String,
    pub model: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAppearance {
    pub skin: Option<SkinPreview>,
    pub local_only: bool,
}
fn failure(message: &str) -> BoxError {
    io::Error::other(message).into()
}
fn directory(uuid: &str) -> Result<PathBuf, BoxError> {
    if uuid.len() != 32 || !uuid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(failure("玩家 UUID 无效"));
    }
    Ok(dirs::config_dir()
        .ok_or_else(|| failure("无法找到设置目录"))?
        .join("CobbleLauncher/skins")
        .join(uuid))
}
fn decode_skin(bytes: &[u8]) -> Result<DynamicImage, BoxError> {
    if bytes.len() > 1024 * 1024 {
        return Err(failure("皮肤文件不能超过 1 MiB"));
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(64);
    limits.max_image_height = Some(64);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| failure("请选择有效的 64×64 或 64×32 PNG 皮肤"))?;
    if image.width() != 64 || !matches!(image.height(), 32 | 64) {
        return Err(failure("皮肤尺寸须为 64×64 或 64×32"));
    }
    Ok(image)
}
pub fn preview_skin(bytes: &[u8], model: &str) -> Result<SkinPreview, BoxError> {
    if !matches!(model, "classic" | "slim") {
        return Err(failure("请选择宽模型或细模型"));
    }
    let image = decode_skin(bytes)?;
    let mut face = image.crop_imm(8, 8, 8, 8).to_rgba8();
    image::imageops::overlay(&mut face, &image.crop_imm(40, 8, 8, 8).to_rgba8(), 0, 0);
    let mut avatar = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(face).write_to(&mut avatar, ImageFormat::Png)?;
    Ok(SkinPreview {
        image: format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
        avatar: format!(
            "data:image/png;base64,{}",
            STANDARD.encode(avatar.into_inner())
        ),
        model: model.into(),
    })
}
fn read_local(uuid: &str) -> Option<SkinPreview> {
    let root = directory(uuid).ok()?;
    let model = std::fs::read_to_string(root.join("model")).ok()?;
    let mut bytes = Vec::new();
    std::fs::File::open(root.join("skin.png"))
        .ok()?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    preview_skin(&bytes, model.trim()).ok()
}
fn store(uuid: &str, bytes: &[u8], model: &str) -> Result<(), BoxError> {
    let root = directory(uuid)?;
    std::fs::create_dir_all(&root)?;
    let mut file = tempfile::NamedTempFile::new_in(&root)?;
    use std::io::Write;
    file.write_all(bytes)?;
    file.persist(root.join("skin.png"))
        .map_err(|error| error.error)?;
    std::fs::write(root.join("model"), model)?;
    Ok(())
}
fn texture_url(value: &str) -> Result<String, BoxError> {
    let mut url = url::Url::parse(value)?;
    if url.host_str() != Some("textures.minecraft.net")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.scheme(), "http" | "https")
    {
        return Err(failure("皮肤来源无效"));
    }
    let digest = url
        .path()
        .strip_prefix("/texture/")
        .ok_or_else(|| failure("皮肤来源无效"))?;
    if digest.is_empty()
        || digest.len() > 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(failure("皮肤来源无效"));
    }
    url.set_scheme("https")
        .map_err(|_| failure("皮肤来源无效"))?;
    Ok(url.into())
}
pub async fn get_profile_appearance(
    id: &str,
    refresh: bool,
) -> Result<ProfileAppearance, BoxError> {
    let profile = get_player_profile(Some(id))?;
    if !refresh || profile.kind == "offline" {
        if let Some(skin) = read_local(&profile.uuid) {
            if profile.kind == "microsoft"
                && directory(&profile.uuid)
                    .ok()
                    .and_then(|root| std::fs::metadata(root.join("skin.png")).ok())
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(300))
            {
                let profile = profile.clone();
                tokio::spawn(async move {
                    let _ = fetch_profile_appearance(profile).await;
                });
            }
            return Ok(ProfileAppearance {
                skin: Some(skin),
                local_only: profile.kind == "offline",
            });
        }
        if profile.kind == "offline" {
            return Ok(ProfileAppearance {
                skin: None,
                local_only: true,
            });
        }
    }
    fetch_profile_appearance(profile).await
}
async fn fetch_profile_appearance(
    profile: crate::PlayerProfile,
) -> Result<ProfileAppearance, BoxError> {
    let _operation = SKIN_OPERATION.lock().await;
    let identity = launch_identity(Some(&profile.id)).await?;
    let token = identity.access_token().to_owned();
    let response =
        tokio::task::spawn_blocking(move || Http.send(Endpoint::Profile, None, Some(&token)))
            .await??;
    if response.status != 200 {
        return Err(failure("无法读取正版皮肤，请稍后刷新"));
    }
    let value: serde_json::Value = serde_json::from_slice(&response.body)?;
    let Some(skin) = value["skins"]
        .as_array()
        .and_then(|skins| skins.iter().find(|skin| skin["state"] == "ACTIVE"))
    else {
        clear_local(&profile.uuid);
        return Ok(ProfileAppearance {
            skin: None,
            local_only: false,
        });
    };
    let url = texture_url(
        skin["url"]
            .as_str()
            .ok_or_else(|| failure("皮肤来源无效"))?,
    )?;
    let model = if skin["variant"]
        .as_str()
        .is_some_and(|value| value.eq_ignore_ascii_case("slim"))
    {
        "slim"
    } else {
        "classic"
    };
    let broker = cobble_net::Broker::new(Default::default())?;
    let response = broker
        .get_public_metadata(&url, "profile-textures", 1024 * 1024)
        .await?;
    if response.metadata.status != 200 {
        return Err(failure("无法下载皮肤，请稍后刷新"));
    }
    let preview = preview_skin(&response.body, model)?;
    store(&profile.uuid, &response.body, model)?;
    Ok(ProfileAppearance {
        skin: Some(preview),
        local_only: false,
    })
}
pub(crate) fn clear_local(uuid: &str) {
    if let Ok(root) = directory(uuid) {
        for name in ["skin.png", "model"] {
            let _ = std::fs::remove_file(root.join(name));
        }
        let _ = std::fs::remove_dir(root);
    }
}
pub async fn save_profile_skin(
    id: &str,
    bytes: Vec<u8>,
    model: &str,
) -> Result<ProfileAppearance, BoxError> {
    let _operation = SKIN_OPERATION.lock().await;
    let preview = preview_skin(&bytes, model)?;
    let profile = get_player_profile(Some(id))?;
    if profile.kind == "microsoft" {
        let identity = launch_identity(Some(id)).await?;
        let token = identity.access_token().to_owned();
        let upload = bytes.clone();
        let variant = model.to_owned();
        tokio::task::spawn_blocking(move || upload_skin(&token, &upload, &variant)).await??;
    } else if profile.kind != "offline" {
        return Err(failure("不支持这个账号类型"));
    }
    store(&profile.uuid, &bytes, model)?;
    Ok(ProfileAppearance {
        skin: Some(preview),
        local_only: profile.kind == "offline",
    })
}
fn upload_skin(token: &str, bytes: &[u8], model: &str) -> Result<(), BoxError> {
    let mut easy = Easy::new();
    easy.url("https://api.minecraftservices.com/minecraft/profile/skins")?;
    easy.follow_location(false)?;
    easy.netrc(NetRc::Ignored)?;
    easy.connect_timeout(Duration::from_secs(10))?;
    easy.timeout(Duration::from_secs(30))?;
    let mut headers = List::new();
    headers.append(&format!("Authorization: Bearer {token}"))?;
    easy.http_headers(headers)?;
    let mut form = Form::new();
    form.part("variant").contents(model.as_bytes()).add()?;
    form.part("file")
        .buffer("skin.png", bytes.to_vec())
        .content_type("image/png")
        .add()?;
    easy.httppost(form)?;
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|chunk| Ok(chunk.len()))?;
        transfer
            .perform()
            .map_err(|_| failure("皮肤上传连接失败，请重试"))?;
    }
    match easy.response_code()? {
        200..=299 => Ok(()),
        401 => Err(failure("登录已失效，请重新登录")),
        429 => Err(failure("皮肤修改过于频繁，请稍后再试")),
        _ => Err(failure("皮肤修改失败，请稍后重试")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn skin(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(width, height)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }
    #[test]
    fn validates_skin_dimensions_and_models() {
        assert!(preview_skin(&skin(64, 64), "classic").is_ok());
        assert!(preview_skin(&skin(64, 32), "slim").is_ok());
        assert!(preview_skin(&skin(128, 128), "classic").is_err());
        assert!(preview_skin(b"not png", "classic").is_err());
        assert!(preview_skin(&skin(64, 64), "other").is_err());
    }
    #[test]
    fn texture_fetch_only_accepts_official_public_origin() {
        assert_eq!(
            texture_url("http://textures.minecraft.net/texture/aabb").unwrap(),
            "https://textures.minecraft.net/texture/aabb"
        );
        for url in [
            "https://example.com/texture/aabb",
            "https://textures.minecraft.net/profile",
            "https://user@textures.minecraft.net/texture/aabb",
            "https://textures.minecraft.net/texture/../x",
        ] {
            assert!(texture_url(url).is_err());
        }
    }
}
