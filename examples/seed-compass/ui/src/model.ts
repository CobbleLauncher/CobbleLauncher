// SPDX-License-Identifier: MIT
export function decimalSeed(value: string): string | null {
    if (!/^-?\d{1,20}$/.test(value.trim()))
        return null;
    try {
        const seed = BigInt(value.trim());
        return seed >= -(1n << 63n) && seed <= (1n << 63n) - 1n ? seed.toString() : null;
    }
    catch {
        return null;
    }
}
export function blockCoordinate(value: string): number | null {
    if (!/^-?\d{1,8}$/.test(value.trim()))
        return null;
    const result = Number(value);
    return Number.isSafeInteger(result) && Math.abs(result) <= 29999000 ? result : null;
}
export function worldRows(value: unknown): {
    id: string;
    name: string;
}[] {
    if (!value || typeof value !== "object" || !("items" in value) || !Array.isArray(value.items))
        return [];
    return value.items.flatMap(row => row && typeof row === "object" && typeof row.id === "string" && typeof row.name === "string" ? [{ id: row.id, name: row.name }] : []);
}
export type Raster = {
    width: number;
    height: number;
    x: number;
    z: number;
    step: number;
    biomes: number[];
    palette: {
        id: number;
        name: string;
        color: string;
    }[];
};
export function rasterPixels(map: Raster): Uint8ClampedArray<ArrayBuffer> {
    if (!Number.isInteger(map.width) || !Number.isInteger(map.height) || map.width < 1 || map.height < 1 || map.width > 128 || map.height > 128 || map.biomes.length !== map.width * map.height)
        throw Error("SEED_MAP_INVALID_RASTER");
    const palette = new Map(map.palette.map(item => [item.id, item]));
    const output = new Uint8ClampedArray(map.biomes.length * 4);
    map.biomes.forEach((id, index) => {
        const color = palette.get(id)?.color;
        if (!color || !/^#[a-f\d]{6}$/i.test(color))
            throw Error("SEED_MAP_INVALID_PALETTE");
        const rgb = parseInt(color.slice(1), 16);
        output.set([rgb >> 16, (rgb >> 8) & 255, rgb & 255, 255], index * 4);
    });
    return output;
}
export function mapPoint(map: Pick<Raster, "width" | "height" | "x" | "z" | "step">, left: number, top: number): {
    x: number;
    z: number;
} {
    return { x: map.x + Math.min(map.width - 1, Math.max(0, Math.floor(left * map.width))) * map.step, z: map.z + Math.min(map.height - 1, Math.max(0, Math.floor(top * map.height))) * map.step };
}

export type VersionCatalog = import('../../../../packages/plugin-web-sdk/src/seed-map').SeedMapVersionCatalog;
type CatalogSource = {versionCatalogs?: readonly VersionCatalog[]; supportedVersions?: readonly string[]; biomes?: VersionCatalog['biomes']; structures?: {id:string;dimension:import('../../../../packages/plugin-web-sdk/src/seed-map').SeedMapDimension;supportedVersions:string[]}[]};
export function versionCatalog(capabilities: CatalogSource | null, version: string): VersionCatalog | null {
    if (capabilities?.versionCatalogs) return capabilities.versionCatalogs.find(catalog => catalog.version === version) ?? null;
    // Compatibility with the original host's three explicit modern releases and fixed height contract.
    if (!['1.20.6','1.21.1','1.21.3'].includes(version) || !capabilities?.supportedVersions?.includes(version) || !capabilities.biomes || !capabilities.structures) return null;
    return {version,engineVersion:version,mapping:'exact',biomes:capabilities.biomes,structures:capabilities.structures.filter(item => item.supportedVersions.includes(version)),yBounds:{overworld:{min:-64,max:319},nether:{min:0,max:255},end:{min:0,max:255}}};
}
export function versionTargets(catalog: VersionCatalog | null, dimension: string) {
    return {biomes: catalog?.biomes.filter(item => item.dimension === dimension) ?? [], structures: catalog?.structures.filter(item => item.dimension === dimension) ?? []};
}
export function versionHeight(catalog: VersionCatalog | null, dimension: string, value: string) {
    const bounds = catalog?.yBounds[dimension as keyof VersionCatalog['yBounds']];
    if (!bounds || !Number.isInteger(bounds.min) || !Number.isInteger(bounds.max) || bounds.min > bounds.max) return null;
    const numeric = Number(value);
    return {...bounds, value: Number.isInteger(numeric) ? Math.min(bounds.max, Math.max(bounds.min, numeric)) : bounds.min};
}
