// SPDX-License-Identifier: MIT
export type Localized = string | {
    en: string;
    'zh-CN'?: string;
};
export type Field = {
    key: string;
    label?: Localized;
    format?: 'bytes' | 'date' | 'count';
};
export type Row = Record<string, unknown>;
export type Dataset = {
    key: string;
    rows: Row[];
};
export function localized(value: Localized, zh: boolean): string { return typeof value === 'string' ? value : zh ? value['zh-CN'] ?? value.en : value.en; }
export function humanize(value: string): string { return value.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/[_-]/g, ' ').replace(/^./, c => c.toUpperCase()); }
export function lookup(value: unknown, key: string): unknown { return key.split('.').reduce<unknown>((current, part) => current && typeof current === 'object' ? (current as Row)[part] : undefined, value); }
export function format(value: unknown, kind?: Field['format'], zh = false): string {
    if (value === undefined || value === null || value === '')
        return '—';
    if (kind === 'bytes' && typeof value === 'number') {
        if (value < 1024)
            return `${value} B`;
        for (const [limit, label] of [[1024 ** 3, 'GiB'], [1024 ** 2, 'MiB'], [1024, 'KiB']] as const)
            if (value >= limit)
                return `${(value / limit).toFixed(1)} ${label}`;
    }
    if (kind === 'date' && typeof value === 'number') {
        const time = new Date(value > 1e12 ? value : value * 1000);
        return Number.isNaN(time.getTime()) ? '—' : time.toLocaleString(zh ? 'zh-CN' : 'en');
    }
    if (typeof value === 'boolean')
        return zh ? (value ? '是' : '否') : (value ? 'Yes' : 'No');
    if (Array.isArray(value))
        return value.map(item => typeof item === 'object' ? JSON.stringify(item) : String(item)).join(', ');
    if (typeof value === 'object')
        return JSON.stringify(value);
    return String(value);
}
export function datasets(data: unknown): Dataset[] {
    if (Array.isArray(data))
        return [{ key: 'items', rows: data.map(value => value && typeof value === 'object' ? value as Row : { value }) }];
    if (!data || typeof data !== 'object')
        return [];
    return Object.entries(data).filter(([, value]) => Array.isArray(value)).map(([key, value]) => ({ key, rows: (value as unknown[]).map(row => row && typeof row === 'object' ? row as Row : { value: row }) }));
}
export function columns(rows: Row[]): Field[] { const sample = rows.slice(0, 20); const keys = [...new Set(sample.flatMap(row => Object.keys(row)))]; return keys.filter(key => !['id', 'path', 'absolutePath', 'directory'].includes(key) && sample.every(row => row[key] === null || typeof row[key] !== 'object')).sort((left, right) => left === 'name' ? -1 : right === 'name' ? 1 : 0).slice(0, 6).map(key => ({ key, format: /bytes|size/i.test(key) ? 'bytes' : /At$/.test(key) ? 'date' : undefined })); }
export function filterRows(rows: Row[], query: string): Row[] { const needle = query.trim().toLocaleLowerCase(); return needle ? rows.filter(row => Object.values(row).some(value => format(value).toLocaleLowerCase().includes(needle))) : rows; }
