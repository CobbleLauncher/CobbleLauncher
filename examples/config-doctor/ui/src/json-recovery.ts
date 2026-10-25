// SPDX-License-Identifier: MIT
export type JsonFieldChange = {
  path: string;
  beforeExists: boolean;
  before: unknown;
  currentExists: boolean;
  current: unknown;
};
type JsonObject = { [key: string]: unknown };
const object = (value: unknown): value is JsonObject => !!value && typeof value === 'object' && !Array.isArray(value);
function equal(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) && Array.isArray(right)) return left.length === right.length && left.every((item, index) => equal(item, right[index]));
  if (!object(left) || !object(right)) return false;
  const a = Object.keys(left).sort(), b = Object.keys(right).sort();
  return a.length === b.length && a.every((key, index) => key === b[index] && equal(left[key], right[key]));
}
const pointerPart = (value: string) => value.replaceAll('~', '~0').replaceAll('/', '~1');
const decodePart = (value: string) => value.replaceAll('~1', '/').replaceAll('~0', '~');

/** Field-level recovery is limited to valid JSON object roots. Unsupported/invalid formats return null. */
export function jsonFieldChanges(currentText: string, beforeText: string): JsonFieldChange[] | null {
  try {
    const current: unknown = JSON.parse(currentText.replace(/^\uFEFF/u, ''));
    const before: unknown = JSON.parse(beforeText.replace(/^\uFEFF/u, ''));
    if (!object(current) || !object(before)) return null;
    const changes: JsonFieldChange[] = [];
    const visit = (path: string, oldValue: unknown, oldExists: boolean, newValue: unknown, newExists: boolean) => {
      if (oldExists && newExists && equal(oldValue, newValue)) return;
      if (oldExists && newExists && object(oldValue) && object(newValue)) {
        const keys = [...new Set([...Object.keys(oldValue), ...Object.keys(newValue)])].sort();
        if (!keys.length) changes.push({ path, beforeExists: true, before: oldValue, currentExists: true, current: newValue });
        else for (const key of keys) visit(`${path}/${pointerPart(key)}`, oldValue[key], Object.hasOwn(oldValue, key), newValue[key], Object.hasOwn(newValue, key));
        return;
      }
      changes.push({ path, beforeExists: oldExists, before: oldValue, currentExists: newExists, current: newValue });
    };
    for (const key of [...new Set([...Object.keys(before), ...Object.keys(current)])].sort()) {
      visit(`/${pointerPart(key)}`, before[key], Object.hasOwn(before, key), current[key], Object.hasOwn(current, key));
    }
    return changes;
  } catch {
    return null;
  }
}

export function restoreJsonFields(currentText: string, beforeText: string, paths: readonly string[]): string {
  const current: unknown = JSON.parse(currentText.replace(/^\uFEFF/u, ''));
  const before: unknown = JSON.parse(beforeText.replace(/^\uFEFF/u, ''));
  if (!object(current) || !object(before)) throw new Error('JSON_RECOVERY_UNSUPPORTED');
  for (const path of paths) {
    const parts = path.split('/').slice(1).map(decodePart);
    if (!parts.length || !parts[0]) throw new Error('JSON_RECOVERY_PATH_INVALID');
    const last = parts.pop()!;
    let target: JsonObject = current;
    let source: unknown = before;
    let valid = true;
    for (const part of parts) {
      if (!object(source) || !Object.hasOwn(source, part)) { valid = false; break; }
      source = source[part];
      if (!object(target[part])) target[part] = {};
      target = target[part] as JsonObject;
    }
    if (!valid) continue;
    if (!object(source)) throw new Error('JSON_RECOVERY_PATH_INVALID');
    if (Object.hasOwn(source, last)) target[last] = structuredClone(source[last]);
    else delete target[last];
  }
  const indentation = currentText.match(/\n([ \t]+)["}]/u)?.[1].length ?? 2;
  const bom = currentText.startsWith('\uFEFF') ? '\uFEFF' : '';
  const newline = /\r\n/u.test(currentText) ? '\r\n' : '\n';
  let result = JSON.stringify(current, null, indentation);
  if (newline === '\r\n') result = result.replaceAll('\n', '\r\n');
  return `${bom}${result}${currentText.endsWith('\n') ? newline : ''}`;
}
