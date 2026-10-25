// SPDX-License-Identifier: MIT
import type { LogRedaction } from '../../../../packages/plugin-web-sdk/src/redaction';
export type LogView = 'redacted' | 'changes' | 'loaded';
export type PreviewLine = { number: number; text: string; marks: { start: number; end: number }[] };
export function logPreview(loaded: string, result: LogRedaction, mode: LogView, query: string): { lines: PreviewLine[]; limited: boolean } {
  const text = mode === 'loaded' ? loaded : result.text, needle = query.trim().toLocaleLowerCase();
  const changed = new Map<number, { start: number; end: number }[]>();
  for (const change of result.changes) changed.set(change.line, [...(changed.get(change.line) ?? []), change]);
  const lines: PreviewLine[] = []; let offset = 0, bytes = 0, limited = false, number = 0;
  for (const segment of text.split('\n')) {
    number++; const value = segment.replace(/\r$/u, ''), start = offset; offset += segment.length + 1;
    if (mode === 'changes' && !changed.has(number) || needle && !value.toLocaleLowerCase().includes(needle)) continue;
    bytes += new TextEncoder().encode(value).length;
    if (lines.length >= 400 || bytes > 65_536) { limited = true; break; }
    const marks = mode === 'loaded' ? [] : (changed.get(number) ?? []).map(change => ({ start: Math.max(0, change.start - start), end: Math.min(value.length, change.end - start) }));
    lines.push({ number, text: value, marks });
  }
  return { lines, limited: limited || mode === 'changes' && result.changesTruncated };
}
export function sharingLimits(text: string): { copy: boolean; export: boolean } {
  const bytes = new TextEncoder().encode(text).length;
  return { copy: bytes > 0 && bytes <= 8192 && !text.includes('\0'), export: bytes > 0 && bytes <= 1_048_576 };
}
