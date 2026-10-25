// SPDX-License-Identifier: MIT
import type { Report } from '../../../../packages/plugin-web-sdk/src/index';
import type { LanguageSelection } from '../../../../packages/plugin-web-sdk/src/lang-package';
export const KINDS = ['missing_key', 'extra_key', 'placeholder_mismatch', 'duplicate_key', 'invalid_json', 'invalid_language'] as const;
export type Issue = { kind: string; key: string; line: number | null; reference: string; translation: string; message: string; file: string };
export function issuesOf(report: Report): Issue[] {
    const table = report.tables.find(item => ['kind', 'key', 'line', 'file'].every(column => item.columns.includes(column)));
    if (!table) return [];
    const column = (row: string[], name: string) => row[table.columns.indexOf(name)] ?? '';
    return table.rows.map(row => {
        const line = column(row, 'line');
        return { kind: column(row, 'kind'), key: column(row, 'key'), line: /^[1-9][0-9]{0,7}$/.test(line) ? Number(line) : null, reference: column(row, 'reference'), translation: column(row, 'translation'), message: column(row, 'message'), file: column(row, 'file') };
    });
}
export function metadataOf(report: Report): Record<string, string> {
    const table = report.tables.find(item => item.columns.includes('reference_keys') && item.columns.includes('issue_count'));
    return Object.fromEntries(table?.columns.map((column, index) => [column, table.rows[0]?.[index] ?? '']) ?? []);
}
export function visibleIssues(issues: Issue[], query: string, kind: string): Issue[] {
    const needle = query.trim().toLocaleLowerCase();
    return issues.filter(issue => (!kind || issue.kind === kind) && `${issue.key} ${issue.reference} ${issue.translation} ${issue.message}`.toLocaleLowerCase().includes(needle));
}
export function localeChoices(selection: LanguageSelection, namespace: string | null): string[] {
    return [...new Set(selection.files.filter(file => file.namespace === namespace).map(file => file.locale))];
}
export function defaultLocales(locales: string[]): { baseline: string; target: string } {
    if (locales.length < 2) return { baseline: '', target: locales[0] ?? '' };
    const baseline = locales.includes('en_us') ? 'en_us' : locales[0]!;
    return { baseline, target: locales.find(locale => locale === 'zh_cn' && locale !== baseline) ?? locales.find(locale => locale !== baseline)! };
}
export function sourceLines(text: string, line: number | null): { number: number; text: string; active: boolean }[] {
    const lines = text.split(/\r?\n/u), selected = line && line <= lines.length ? line : 1;
    const start = Math.max(0, selected - 6), end = Math.min(lines.length, selected + 5);
    return lines.slice(start, end).map((text, index) => ({ number: start + index + 1, text, active: start + index + 1 === line }));
}
export function reportText(report: Report, source: string): string {
    return [source, ...report.findings.map(finding => `${finding.title}\n${finding.message}`), ...report.tables.map(table => [table.columns.join('\t'), ...table.rows.map(row => row.join('\t'))].join('\n'))].join('\n\n');
}
export function kindName(kind: string, zh: boolean): string {
    return ({ missing_key: ['Missing translation', '缺少翻译'], extra_key: ['Extra key', '多余键'], placeholder_mismatch: ['Placeholder mismatch', '占位符不一致'], duplicate_key: ['Duplicate key', '重复键'], invalid_json: ['Invalid JSON', 'JSON 错误'], invalid_language: ['Invalid translation value', '翻译值格式错误'] } as Record<string, string[]>)[kind]?.[zh ? 1 : 0] ?? kind;
}
