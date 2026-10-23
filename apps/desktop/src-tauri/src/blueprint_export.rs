// SPDX-License-Identifier: MIT
//! Export the host-held summary; renderer input never supplies cells or paths.
use cobble_plugin_api::MaterialCount;
use tauri::WebviewWindow;

#[derive(Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialExportFormat {
    #[default]
    Csv,
    Text,
}

impl MaterialExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Text => "txt",
        }
    }
}

#[tauri::command]
pub async fn export_blueprint_materials(
    window: WebviewWindow,
    resource_id: String,
    format: Option<MaterialExportFormat>,
) -> Result<bool, String> {
    let format = format.unwrap_or_default();
    let resource = super::resource(&resource_id)?;
    let materials = resource
        .session
        .lock()
        .map_err(|error| error.to_string())?
        .summary()
        .materials
        .clone();
    let filename = suggested_filename(&resource.name, format);
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_file_name(&filename)
        .add_filter(format.extension(), &[format.extension()])
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let path = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let contents = match format {
            MaterialExportFormat::Csv => material_csv(&materials),
            MaterialExportFormat::Text => material_text(&materials),
        };
        std::fs::write(path, contents).map_err(|error| error.to_string())?;
        Ok(true)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn suggested_filename(name: &str, format: MaterialExportFormat) -> String {
    let stem = [".schem", ".litematic"]
        .iter()
        .find_map(|extension| {
            name.to_ascii_lowercase()
                .ends_with(extension)
                .then(|| &name[..name.len() - extension.len()])
        })
        .unwrap_or(name);
    let safe: String = stem
        .chars()
        .map(|ch| {
            if ch.is_control() || "<>:\"/\\|?*".contains(ch) {
                '_'
            } else {
                ch
            }
        })
        .take(100)
        .collect();
    let safe = safe.trim_matches([' ', '.']);
    if safe.is_empty() {
        format!("blueprint-materials.{}", format.extension())
    } else {
        format!("{safe}-materials.{}", format.extension())
    }
}

fn material_text(materials: &[MaterialCount]) -> String {
    let mut text = String::new();
    for item in materials {
        text.push_str(&item.count.to_string());
        text.push_str(" × ");
        // Keep each material on one line, even for malformed guest labels.
        text.extend(
            item.state
                .chars()
                .map(|ch| if ch.is_control() { ' ' } else { ch }),
        );
        text.push_str("\r\n");
    }
    text
}

fn material_csv(materials: &[MaterialCount]) -> String {
    let mut csv = String::from("block,count\r\n");
    for item in materials {
        // Quoting alone does not prevent spreadsheet formulas. Prefix suspicious
        // values (including leading whitespace) with an apostrophe as text.
        let formula = item.state.trim_start().starts_with(['=', '+', '-', '@']);
        csv.push('"');
        if formula || item.state.starts_with(['\t', '\r', '\n']) {
            csv.push('\'');
        }
        csv.push_str(&item.state.replace('"', "\"\""));
        csv.push_str("\",");
        csv.push_str(&item.count.to_string());
        csv.push_str("\r\n");
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_preserves_states_counts_and_escapes_delimiters() {
        let rows = vec![
            MaterialCount {
                state: "minecraft:oak_stairs[facing=east,half=top]".into(),
                count: 42,
            },
            MaterialCount {
                state: "a\"b\r\nc".into(),
                count: 1,
            },
        ];
        assert_eq!(
            material_csv(&rows),
            "block,count\r\n\"minecraft:oak_stairs[facing=east,half=top]\",42\r\n\"a\"\"b\r\nc\",1\r\n"
        );
        assert_eq!(material_csv(&[]), "block,count\r\n");
    }

    #[test]
    fn spreadsheet_formulas_are_exported_as_text() {
        for value in [
            "=1+1",
            "+cmd",
            "-2+3",
            "@SUM(A1)",
            "  =1",
            "\tformula",
            "\rformula",
            "\nformula",
        ] {
            let csv = material_csv(&[MaterialCount {
                state: value.into(),
                count: 7,
            }]);
            assert_eq!(csv, format!("block,count\r\n\"'{value}\",7\r\n"));
        }
    }

    #[test]
    fn suggested_names_cannot_supply_a_directory() {
        assert_eq!(
            suggested_filename("Castle.schem", MaterialExportFormat::Csv),
            "Castle-materials.csv"
        );
        assert_eq!(
            suggested_filename("../folder\\test.schem", MaterialExportFormat::Csv),
            "_folder_test-materials.csv"
        );
        assert_eq!(
            suggested_filename("...", MaterialExportFormat::Text),
            "blueprint-materials.txt"
        );
    }

    #[test]
    fn text_preserves_exact_counts_and_states_without_inventing_stack_sizes() {
        assert_eq!(
            material_text(&[MaterialCount {
                state: "minecraft:oak_stairs[facing=east,half=top]".into(),
                count: 130
            }]),
            "130 × minecraft:oak_stairs[facing=east,half=top]\r\n"
        );
        assert_eq!(
            material_text(&[MaterialCount {
                state: "bad\nlabel\t".into(),
                count: 1
            }]),
            "1 × bad label \r\n"
        );
        assert!(material_text(&[]).is_empty());
        assert!(serde_json::from_str::<MaterialExportFormat>("\"html\"").is_err());
    }
}
