// SPDX-License-Identifier: MIT
/** Last saved local level.dat metadata; never a remote server or the currently open world. */
export type LocalWorldMetadata = {
  schemaVersion: 1; worldId: string; source: 'saved_metadata'; seed: string | null;
  seedSource: 'world_gen_settings' | 'random_seed' | null; version: string | null; dataVersion: number | null;
  generator: { preset: string | null; custom: boolean | null; hints: string[] };
  spawn: { x: number; y: number; z: number } | null;
  unavailable: 'metadata_unavailable' | 'metadata_invalid' | 'metadata_too_large' | 'metadata_changed' | 'seed_missing' | null;
};
export type SeedMapDimension = 'overworld' | 'nether' | 'end';
export type SeedMapPreset = 'default' | 'large_biomes';
export type SeedMapRequest = { seed: string; version: string; dimension: SeedMapDimension; preset: SeedMapPreset; y: number; x: number; z: number; width: number; height: number; step: number };
export type SeedSearchRequest = { seed: string; version: string; dimension: SeedMapDimension; preset: SeedMapPreset; y: number; x: number; z: number; radius: number; target: { kind: 'biome' | 'structure'; id: string }; limit: number };
export type SeedMapBiome = { id: number; name: string; color: string; dimension: SeedMapDimension };
export type SeedMapVersionCatalog = {
  version: string; engineVersion: string; mapping: 'exact' | 'family';
  biomes: SeedMapBiome[]; structures: { id: string; dimension: SeedMapDimension }[];
  yBounds: Record<SeedMapDimension, { min: number; max: number }>;
};
export type SeedMapCapabilities = { engine: string; engineRevision: string; supportedVersions: string[]; versionCatalogs?: SeedMapVersionCatalog[]; dimensions: SeedMapDimension[]; presets: SeedMapPreset[]; biomes: SeedMapBiome[]; structures: { id: string; dimension: SeedMapDimension; supportedVersions: string[] }[]; maxWidth: number; maxHeight: number; steps: number[]; maxRadius: number; biomeSearchStep: number; maxResults: number };
export type SeedMapResult = { width: number; height: number; x: number; z: number; step: number; y: number; biomes: number[]; palette: SeedMapBiome[]; engineRevision: string };
export type SeedSearchResult = { matches: { x: number; z: number; distance: number; kind: 'biome' | 'structure'; id: string; confidence: 'sampled_biome' | 'biome_checked_terrain_unverified' }[]; sampledStep: number | null; engineRevision: string };
