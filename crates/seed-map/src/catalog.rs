// SPDX-License-Identifier: MIT
//! Explicit upstream version mappings and version-scoped prediction catalogs.
use crate::{Biome, Capabilities, ENGINE_REVISION, SeedMapError, Structure, error, ffi};
use serde::Serialize;

pub(crate) struct Version {
    pub name: &'static str,
    pub code: i32,
    engine: &'static str,
    mapping: &'static str,
}
pub(crate) const VERSIONS: &[Version] = &[
    Version {
        name: "1.12.2",
        code: 11202,
        engine: "1.12.2",
        mapping: "exact",
    },
    Version {
        name: "1.16.1",
        code: 11601,
        engine: "1.16.1",
        mapping: "exact",
    },
    Version {
        name: "1.16.5",
        code: 11605,
        engine: "1.16.5",
        mapping: "exact",
    },
    Version {
        name: "1.17.1",
        code: 11701,
        engine: "1.17.1",
        mapping: "exact",
    },
    Version {
        name: "1.18.2",
        code: 11802,
        engine: "1.18.2",
        mapping: "exact",
    },
    Version {
        name: "1.19.2",
        code: 11902,
        engine: "1.19.2",
        mapping: "exact",
    },
    Version {
        name: "1.19.4",
        code: 11904,
        engine: "1.19.4",
        mapping: "exact",
    },
    Version {
        name: "1.20.1",
        code: 12001,
        engine: "1.20",
        mapping: "family",
    },
    Version {
        name: "1.20.6",
        code: 12006,
        engine: "1.20.6",
        mapping: "exact",
    },
    Version {
        name: "1.21.1",
        code: 12101,
        engine: "1.21.1",
        mapping: "exact",
    },
    Version {
        name: "1.21.3",
        code: 12103,
        engine: "1.21.3",
        mapping: "exact",
    },
];
pub(crate) const STRUCTURES: &[(&str, i32)] = &[
    ("desert_pyramid", 1),
    ("jungle_pyramid", 2),
    ("swamp_hut", 3),
    ("igloo", 4),
    ("village", 5),
    ("ocean_ruin", 6),
    ("shipwreck", 7),
    ("monument", 8),
    ("mansion", 9),
    ("pillager_outpost", 10),
    ("ruined_portal", 11),
    ("ruined_portal_nether", 12),
    ("ancient_city", 13),
    ("fortress", 18),
    ("bastion_remnant", 19),
    ("end_city", 20),
    ("trail_ruins", 23),
    ("trial_chambers", 24),
];

#[derive(Clone, Debug, Serialize)]
pub struct YRange {
    pub min: i32,
    pub max: i32,
}
#[derive(Clone, Debug, Serialize)]
pub struct YBounds {
    pub overworld: YRange,
    pub nether: YRange,
    pub end: YRange,
}
#[derive(Clone, Debug, Serialize)]
pub struct CatalogStructure {
    pub id: String,
    pub dimension: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionCatalog {
    pub version: &'static str,
    pub engine_version: &'static str,
    pub mapping: &'static str,
    pub biomes: Vec<Biome>,
    pub structures: Vec<CatalogStructure>,
    pub y_bounds: YBounds,
}

pub(crate) fn version_code(version: &str) -> Result<i32, SeedMapError> {
    VERSIONS
        .iter()
        .find(|v| v.name == version)
        .map(|v| v.code)
        .ok_or_else(|| error("SEED_MAP_UNSUPPORTED_VERSION", "Unsupported Java version"))
}
pub(crate) fn dimension_name(dim: i32) -> &'static str {
    match dim {
        -1 => "nether",
        1 => "end",
        _ => "overworld",
    }
}
pub(crate) fn y_bounds(version: i32) -> YBounds {
    YBounds {
        overworld: if version >= 11802 {
            YRange { min: -64, max: 319 }
        } else {
            YRange { min: 0, max: 255 }
        },
        nether: YRange { min: 0, max: 255 },
        end: YRange { min: 0, max: 255 },
    }
}
pub(crate) fn biome_catalog(version: i32) -> Vec<Biome> {
    (0..256)
        .filter_map(|id| {
            ffi::biome_info(id, version).map(|(name, dim, color)| Biome {
                id,
                name,
                color: format!("#{color:06x}"),
                dimension: dimension_name(dim).into(),
            })
        })
        .collect()
}
pub fn capabilities() -> Capabilities {
    let structures: Vec<Structure> = STRUCTURES
        .iter()
        .filter_map(|(id, kind)| {
            ffi::structure_config(*kind, 12101).map(|(_, dim)| Structure {
                id: (*id).into(),
                dimension: dimension_name(dim).into(),
                supported_versions: VERSIONS
                    .iter()
                    .filter_map(|v| ffi::structure_config(*kind, v.code).map(|_| v.name))
                    .collect(),
            })
        })
        .collect();
    let version_catalogs = VERSIONS
        .iter()
        .map(|v| VersionCatalog {
            version: v.name,
            engine_version: v.engine,
            mapping: v.mapping,
            biomes: biome_catalog(v.code),
            structures: structures
                .iter()
                .filter(|s| s.supported_versions.contains(&v.name))
                .map(|s| CatalogStructure {
                    id: s.id.clone(),
                    dimension: s.dimension.clone(),
                })
                .collect(),
            y_bounds: y_bounds(v.code),
        })
        .collect();
    Capabilities {
        engine: "cubiomes",
        engine_revision: ENGINE_REVISION,
        supported_versions: VERSIONS.iter().map(|v| v.name).collect(),
        version_catalogs,
        dimensions: vec!["overworld", "nether", "end"],
        presets: vec!["default", "large_biomes"],
        biomes: biome_catalog(12101),
        structures,
        max_width: 128,
        max_height: 128,
        steps: vec![4, 8, 16, 32, 64],
        max_radius: 8192,
        biome_search_step: 64,
        max_results: 32,
    }
}
