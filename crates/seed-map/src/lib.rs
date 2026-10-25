// SPDX-License-Identifier: MIT
//! Offline, bounded Java world-generation predictions from pinned MIT Cubiomes.
//! Call on a background worker. No filesystem/network access or persistent seed storage.
mod catalog;
mod ffi;
pub use catalog::{CatalogStructure, VersionCatalog, YBounds, YRange, capabilities};
use catalog::{STRUCTURES, biome_catalog, dimension_name, version_code, y_bounds};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const ENGINE_REVISION: &str = "e61f90580cbdd883214a8054670dacae655e59c0";
const BORDER: i32 = 29_999_984;
const BUDGET: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapRequest {
    pub seed: String,
    pub version: String,
    pub dimension: String,
    pub preset: String,
    pub y: i32,
    pub x: i32,
    pub z: i32,
    pub width: u32,
    pub height: u32,
    pub step: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchRequest {
    pub seed: String,
    pub version: String,
    pub dimension: String,
    pub preset: String,
    pub y: i32,
    pub x: i32,
    pub z: i32,
    pub radius: i32,
    pub target: SearchTarget,
    pub limit: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchTarget {
    pub kind: String,
    pub id: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Biome {
    pub id: i32,
    pub name: String,
    pub color: String,
    pub dimension: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Structure {
    pub id: String,
    pub dimension: String,
    pub supported_versions: Vec<&'static str>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub engine: &'static str,
    pub engine_revision: &'static str,
    pub supported_versions: Vec<&'static str>,
    pub version_catalogs: Vec<VersionCatalog>,
    pub dimensions: Vec<&'static str>,
    pub presets: Vec<&'static str>,
    pub biomes: Vec<Biome>,
    pub structures: Vec<Structure>,
    pub max_width: u32,
    pub max_height: u32,
    pub steps: Vec<i32>,
    pub max_radius: i32,
    pub biome_search_step: i32,
    pub max_results: u32,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapResult {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub z: i32,
    pub step: i32,
    pub y: i32,
    pub biomes: Vec<i32>,
    pub palette: Vec<Biome>,
    pub engine_revision: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct SearchMatch {
    pub x: i32,
    pub z: i32,
    pub distance: f64,
    pub kind: String,
    pub id: String,
    pub confidence: &'static str,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub sampled_step: Option<i32>,
    pub engine_revision: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct SeedMapError {
    pub code: &'static str,
    pub message: &'static str,
}
impl std::fmt::Display for SeedMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}
impl std::error::Error for SeedMapError {}
fn error(code: &'static str, message: &'static str) -> SeedMapError {
    SeedMapError { code, message }
}
fn invalid() -> SeedMapError {
    error(
        "SEED_MAP_INVALID_REQUEST",
        "Request exceeds supported bounds",
    )
}
fn setup(
    seed: &str,
    version: &str,
    dimension: &str,
    preset: &str,
    y: i32,
) -> Result<ffi::Generator, SeedMapError> {
    let version = version_code(version)?;
    let digits = seed.strip_prefix('-').unwrap_or(seed);
    if digits.is_empty() || digits.len() > 19 || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(error(
            "SEED_MAP_INVALID_SEED",
            "Seed must be a signed 64-bit decimal string",
        ));
    }
    let seed = seed.parse::<i64>().map_err(|_| {
        error(
            "SEED_MAP_INVALID_SEED",
            "Seed must be a signed 64-bit decimal string",
        )
    })?;
    let dim = match dimension {
        "overworld" => 0,
        "nether" => -1,
        "end" => 1,
        _ => return Err(invalid()),
    };
    let large = match preset {
        "default" => false,
        "large_biomes" if dim == 0 => true,
        _ => {
            return Err(error(
                "SEED_MAP_UNSUPPORTED_WORLDGEN",
                "Only default or Overworld large-biomes generation is supported",
            ));
        }
    };
    let bounds = y_bounds(version);
    let range = match dim {
        -1 => bounds.nether,
        1 => bounds.end,
        _ => bounds.overworld,
    };
    if !(range.min..=range.max).contains(&y) {
        return Err(invalid());
    }
    ffi::Generator::new(seed, version, dim, large)
        .ok_or_else(|| error("SEED_MAP_ENGINE_FAILED", "Generator allocation failed"))
}
fn checkpoint(cancel: &AtomicBool, started: Instant) -> Result<(), SeedMapError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(error("SEED_MAP_CANCELLED", "Prediction cancelled"));
    }
    if started.elapsed() > BUDGET {
        return Err(error(
            "SEED_MAP_BUDGET_EXCEEDED",
            "Prediction exceeded its compute budget",
        ));
    }
    Ok(())
}
fn bounded(x: i64) -> bool {
    x.abs() <= i64::from(BORDER)
}
pub fn generate(request: &MapRequest, cancel: &AtomicBool) -> Result<MapResult, SeedMapError> {
    let started = Instant::now();
    checkpoint(cancel, started)?;
    if request.width == 0
        || request.width > 128
        || request.height == 0
        || request.height > 128
        || ![4, 8, 16, 32, 64].contains(&request.step)
    {
        return Err(invalid());
    }
    for (origin, count) in [(request.x, request.width), (request.z, request.height)] {
        if !bounded(i64::from(origin))
            || !bounded(i64::from(origin) + i64::from(count - 1) * i64::from(request.step))
        {
            return Err(invalid());
        }
    }
    let g = setup(
        &request.seed,
        &request.version,
        &request.dimension,
        &request.preset,
        request.y,
    )?;
    let mut biomes = Vec::with_capacity((request.width * request.height) as usize);
    for row in 0..request.height {
        checkpoint(cancel, started)?;
        for col in 0..request.width {
            let id = g.biome(
                request.x + col as i32 * request.step,
                request.y,
                request.z + row as i32 * request.step,
            );
            if !(0..256).contains(&id) {
                return Err(error("SEED_MAP_ENGINE_FAILED", "Biome generation failed"));
            }
            biomes.push(id);
        }
    }
    checkpoint(cancel, started)?;
    let ids: BTreeSet<i32> = biomes.iter().copied().collect();
    Ok(MapResult {
        width: request.width,
        height: request.height,
        x: request.x,
        z: request.z,
        step: request.step,
        y: request.y,
        biomes,
        palette: biome_catalog(version_code(&request.version)?)
            .into_iter()
            .filter(|b| ids.contains(&b.id))
            .collect(),
        engine_revision: ENGINE_REVISION,
    })
}
pub fn search(request: &SearchRequest, cancel: &AtomicBool) -> Result<SearchResult, SeedMapError> {
    let started = Instant::now();
    checkpoint(cancel, started)?;
    if !(1..=8192).contains(&request.radius) || !(1..=32).contains(&request.limit) {
        return Err(invalid());
    }
    for origin in [request.x, request.z] {
        if !bounded(i64::from(origin) - i64::from(request.radius))
            || !bounded(i64::from(origin) + i64::from(request.radius))
        {
            return Err(invalid());
        }
    }
    let mut g = setup(
        &request.seed,
        &request.version,
        &request.dimension,
        &request.preset,
        request.y,
    )?;
    let mut matches = Vec::new();
    let mut add = |x: i32, z: i32, confidence| {
        let dx = f64::from(x - request.x);
        let dz = f64::from(z - request.z);
        let distance = dx.hypot(dz);
        if distance <= f64::from(request.radius) {
            matches.push(SearchMatch {
                x,
                z,
                distance,
                kind: request.target.kind.clone(),
                id: request.target.id.clone(),
                confidence,
            });
        }
    };
    let sampled_step = match request.target.kind.as_str() {
        "biome" => {
            let target = biome_catalog(version_code(&request.version)?)
                .into_iter()
                .find(|b| {
                    (b.name == request.target.id || b.id.to_string() == request.target.id)
                        && b.dimension == request.dimension
                })
                .ok_or_else(|| {
                    error(
                        "SEED_MAP_UNSUPPORTED_TARGET",
                        "Biome does not exist in this dimension",
                    )
                })?;
            // A fixed lattice centred on the query point; every reported match is
            // sampled at the exact selected block Y using the full Voronoi zoom.
            let n = request.radius / 64;
            for rz in -n..=n {
                checkpoint(cancel, started)?;
                for rx in -n..=n {
                    let x = request.x + rx * 64;
                    let z = request.z + rz * 64;
                    if g.biome(x, request.y, z) == target.id {
                        add(x, z, "sampled_biome");
                    }
                }
            }
            Some(64)
        }
        "structure" => {
            let kind = STRUCTURES
                .iter()
                .find(|(id, _)| *id == request.target.id)
                .map(|(_, kind)| *kind)
                .ok_or_else(|| error("SEED_MAP_UNSUPPORTED_TARGET", "Unsupported structure"))?;
            let (span, dim) = ffi::structure_config(kind, version_code(&request.version)?)
                .ok_or_else(|| error("SEED_MAP_UNSUPPORTED_TARGET", "Unsupported structure"))?;
            if dimension_name(dim) != request.dimension {
                return Err(error(
                    "SEED_MAP_UNSUPPORTED_TARGET",
                    "Structure does not exist in this dimension",
                ));
            }
            for rz in (request.z - request.radius).div_euclid(span)
                ..=(request.z + request.radius).div_euclid(span)
            {
                for rx in (request.x - request.radius).div_euclid(span)
                    ..=(request.x + request.radius).div_euclid(span)
                {
                    checkpoint(cancel, started)?;
                    if let Some((x, z)) = g.structure(kind, rx, rz) {
                        add(x, z, "biome_checked_terrain_unverified");
                    }
                }
            }
            None
        }
        _ => {
            return Err(error(
                "SEED_MAP_UNSUPPORTED_TARGET",
                "Unsupported target kind",
            ));
        }
    };
    checkpoint(cancel, started)?;
    matches.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then(a.x.cmp(&b.x))
            .then(a.z.cmp(&b.z))
    });
    matches.truncate(request.limit as usize);
    Ok(SearchResult {
        matches,
        sampled_step,
        engine_revision: ENGINE_REVISION,
    })
}

#[cfg(test)]
mod tests;
