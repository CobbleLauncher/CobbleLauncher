// SPDX-License-Identifier: MIT
export type SnapshotOrder = {
    id: string;
    created_at: string | number;
};
export function orderedPair<T extends SnapshotOrder>(items: readonly T[], selection: readonly string[]): [
    T,
    T
] | null {
    const picked = items.filter(item => selection.includes(item.id)).sort((a, b) => snapshotTime(a.created_at) - snapshotTime(b.created_at) || a.id.localeCompare(b.id));
    return picked.length === 2 ? [picked[0]!, picked[1]!] : null;
}
export function toggleSnapshot(selection: readonly string[], id: string): string[] { return selection.includes(id) ? selection.filter(value => value !== id) : selection.length < 2 ? [...selection, id] : [...selection]; }
export function size(value: number | null | undefined): string { if (value == null || !Number.isFinite(value) || value < 0)
    return '—'; const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB']; let unit = 0; while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
} return `${unit ? value.toFixed(1) : value} ${units[unit]}`; }
export function pageRange(offset: number, count: number, total: number): string { return total === 0 ? '0' : `${offset + 1}–${Math.min(offset + count, total)} / ${total}`; }
export function object(value: unknown): Record<string, unknown> { return value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {}; }
export function worldRows(value: unknown): {
    id: string;
    name: string;
}[] { const data = object(value); return Array.isArray(data.items) ? data.items.flatMap(value => { const row = object(value); return typeof row.id === 'string' && typeof row.name === 'string' ? [{ id: row.id, name: row.name }] : []; }) : []; }
export function snapshotTime(value: string | number): number { return typeof value === 'number' ? value * 1000 : Date.parse(value.replace(/\[[^\[\]]+\]$/, '')); }
