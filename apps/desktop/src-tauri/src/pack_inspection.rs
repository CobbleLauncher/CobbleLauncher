// SPDX-License-Identifier: MIT
//! Native picker and bounded immutable inventory; paths never enter guest input.
use cobble_content::{MrpackInspectionSnapshot, inspect_mrpack_archive};
use serde::Serialize;
use std::{
    fs::File,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use tauri::WebviewWindow;

#[derive(Serialize)]
struct Request {
    current: MrpackInspectionSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline: Option<MrpackInspectionSnapshot>,
}

fn snapshot(path: &Path) -> Result<MrpackInspectionSnapshot, String> {
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("mrpack"))
    {
        return Err("请选择 .mrpack 文件".into());
    }
    let file = File::open(path).map_err(|_| "无法读取整合包")?;
    if !file.metadata().map_err(|_| "无法读取整合包")?.is_file() {
        return Err("请选择普通文件".into());
    }
    inspect_mrpack_archive(file).map_err(|error| format!("无法分析整合包：{error}"))
}

pub async fn prepare_pack_report(
    window: &WebviewWindow,
    compare: bool,
) -> Result<Option<(Vec<u8>, &'static str)>, String> {
    Ok(
        prepare_pack_report_for_extensions(window, compare, &["mrpack".into(), "json".into()])
            .await?
            .map(|(bytes, media, _)| (bytes, media)),
    )
}

pub(super) async fn prepare_pack_report_for_extensions(
    window: &WebviewWindow,
    compare: bool,
    allowed: &[String],
) -> Result<Option<(Vec<u8>, &'static str, Duration)>, String> {
    let extensions: Vec<_> = allowed
        .iter()
        .filter(|extension| {
            extension.as_str() == "mrpack" || (!compare && extension.as_str() == "json")
        })
        .cloned()
        .collect();
    if extensions.is_empty() {
        return Err("插件不支持所选整合包分析方式".into());
    }
    let Some(current) = rfd::AsyncFileDialog::new()
        .set_parent(window)
        .set_title("选择整合包")
        .add_filter("Modrinth 整合包", &extensions)
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let baseline = if compare {
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_parent(window)
            .set_title("选择用于比较的旧整合包")
            .add_filter("Modrinth 整合包", &["mrpack"])
            .pick_file()
            .await
        else {
            return Ok(None);
        };
        Some(file.path().to_owned())
    } else {
        None
    };
    let current = current.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let extension = current
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !extensions.contains(&extension) {
            return Err("插件不支持此文件类型".into());
        }
        if current
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("json"))
            && !compare
        {
            let file = File::open(&current).map_err(|_| "无法读取整合包索引")?;
            if !file.metadata().map_err(|_| "无法读取整合包索引")?.is_file() {
                return Err("请选择普通文件".into());
            }
            let mut bytes = Vec::new();
            file.take(cobble_plugin_api::MAX_FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "无法读取整合包索引")?;
            if bytes.len() > cobble_plugin_api::MAX_FILE_BYTES {
                return Err("索引超过 8 MiB 限制".into());
            }
            return Ok(Some((bytes, "application/json", started.elapsed())));
        }
        let request = Request {
            current: snapshot(&current)?,
            baseline: baseline.as_deref().map(snapshot).transpose()?,
        };
        let bytes = serde_json::to_vec(&request).map_err(|_| "无法准备整合包报告")?;
        if bytes.len() > cobble_plugin_api::MAX_FILE_BYTES {
            return Err("整合包分析元数据超过 8 MiB 限制".into());
        }
        Ok(Some((
            bytes,
            "application/vnd.cobble.mrpack-inspection+json",
            started.elapsed(),
        )))
    })
    .await
    .map_err(|_| "无法准备整合包报告".to_owned())?
}
