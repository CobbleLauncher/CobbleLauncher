// SPDX-License-Identifier: MIT
export const DOCUMENT_KEY = 'locations-v1';
export const COORDINATE_LIMIT = 29999984;
export type Dimension = 'overworld' | 'nether' | 'end';
export type Location = {
    id: string;
    name: string;
    dimension: Dimension;
    x: number;
    y: number | null;
    z: number;
    note: string;
};
export type Book = {
    schemaVersion: 1;
    locations: Location[];
};
export type Draft = {
    id: string;
    name: string;
    dimension: Dimension;
    x: string;
    y: string;
    z: string;
    note: string;
};
export function emptyDraft(): Draft {
    return { id: '', name: '', dimension: 'overworld', x: '', y: '', z: '', note: '' };
}
export function draftOf(location: Location): Draft {
    return { ...location, x: String(location.x), y: location.y === null ? '' : String(location.y), z: String(location.z) };
}
export function coordinate(value: string, axis: 'x' | 'y' | 'z'): number | null {
    if (!/^-?\d{1,8}$/.test(value.trim()))
        return null;
    const number = Number(value), limit = axis === 'y' ? 4096 : COORDINATE_LIMIT;
    return Number.isSafeInteger(number) && Math.abs(number) <= limit ? (Object.is(number, -0) ? 0 : number) : null;
}
export function locationOf(draft: Draft, id: string): Location | null {
    const x = coordinate(draft.x, 'x'), z = coordinate(draft.z, 'z'), y = draft.y.trim() === '' ? null : coordinate(draft.y, 'y');
    if (!id || id.length > 80 || !draft.name.trim() || draft.name.trim().length > 80 || draft.note.length > 512 || x === null || z === null || (draft.y.trim() !== '' && y === null) || !['overworld', 'nether', 'end'].includes(draft.dimension))
        return null;
    return { id, name: draft.name.trim(), dimension: draft.dimension, x, y, z, note: draft.note };
}
export function decodeBook(value: unknown): Book {
    if (value === null)
        return { schemaVersion: 1, locations: [] };
    if (!value || typeof value !== 'object' || !('schemaVersion' in value) || value.schemaVersion !== 1 || !('locations' in value) || !Array.isArray(value.locations) || value.locations.length > 128)
        throw Error('BOOK_DOCUMENT_INVALID');
    const ids = new Set<string>();
    const locations = value.locations.map(item => {
        if (!item || typeof item !== 'object' || typeof item.id !== 'string' || typeof item.name !== 'string' || typeof item.dimension !== 'string' || typeof item.x !== 'number' || typeof item.z !== 'number' || (item.y !== null && typeof item.y !== 'number') || typeof item.note !== 'string')
            throw Error('BOOK_DOCUMENT_INVALID');
        const parsed = locationOf(draftOf(item as Location), item.id);
        if (!parsed || ids.has(parsed.id))
            throw Error('BOOK_DOCUMENT_INVALID');
        ids.add(parsed.id);
        return parsed;
    });
    const result = { schemaVersion: 1 as const, locations };
    assertSize(result);
    return result;
}
export function assertSize(book: Book): void {
    if (book.locations.length > 128 || new TextEncoder().encode(JSON.stringify(book)).length > 64 * 1024)
        throw Error('BOOK_DOCUMENT_LIMIT');
}
export function upsert(book: Book, location: Location): Book {
    const result = { schemaVersion: 1 as const, locations: book.locations.some(item => item.id === location.id) ? book.locations.map(item => item.id === location.id ? location : item) : [...book.locations, location] };
    assertSize(result);
    return result;
}
export function without(book: Book, id: string): Book {
    return { schemaVersion: 1, locations: book.locations.filter(item => item.id !== id) };
}
export function coordinates(location: Pick<Location, 'x' | 'y' | 'z'>): string {
    return location.y === null ? `${location.x} ${location.z}` : `${location.x} ${location.y} ${location.z}`;
}
export function converted(location: Pick<Location, 'dimension' | 'x' | 'y' | 'z'>): Pick<Location, 'dimension' | 'x' | 'y' | 'z'> | null {
    if (location.dimension === 'end')
        return null;
    const next = location.dimension === 'overworld' ? { dimension: 'nether' as const, x: Math.floor(location.x / 8), z: Math.floor(location.z / 8), y: location.y } : { dimension: 'overworld' as const, x: location.x * 8, z: location.z * 8, y: location.y };
    return Math.abs(next.x) <= COORDINATE_LIMIT && Math.abs(next.z) <= COORDINATE_LIMIT ? next : null;
}
export function draftDirty(draft: Draft, baseline: Draft): boolean {
    return JSON.stringify(draft) !== JSON.stringify(baseline);
}
export function visibleLocations(book: Book, query: string): Location[] {
    const needle = query.trim().toLocaleLowerCase();
    return book.locations.filter(item => `${item.name} ${item.dimension} ${coordinates(item)} ${item.note}`.toLocaleLowerCase().includes(needle));
}
