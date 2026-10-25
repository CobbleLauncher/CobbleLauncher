// SPDX-License-Identifier: MIT
export const MATERIALS_IMPORT_INTERFACE = 'launcher:materials-import' as const;
export const MATERIALS_IMPORT_VERSION = '0.1.0' as const;
export type MaterialRequirement = { id: string; name?: string; required: number; stackSize?: number | null };
export type MaterialsPayload = { name: string; source?: string; materials: MaterialRequirement[] };
export type MaterialsHandoffDraft = { handoffId: string; sender: {pluginId: string; name: string}; game: {id: string; name: string} | null; worldId?: string | null; payload: MaterialsPayload };
function text(value: unknown, bytes: number, chars: number): string {
  if (typeof value !== 'string' || !value.trim() || [...value].length > chars || new TextEncoder().encode(value).length > bytes || /[\u0000-\u001f\u007f]/u.test(value)) throw new Error('INVALID_MATERIALS');
  return value;
}
export function validateMaterialsPayload(value: MaterialsPayload): MaterialsPayload {
  if (!value || !Array.isArray(value.materials) || !value.materials.length || value.materials.length > 2048) throw new Error('INVALID_MATERIALS');
  const ids = new Set<string>();
  const result = { name: text(value.name, 320, 80), ...(value.source === undefined ? {} : {source: text(value.source, 480, 120)}), materials: value.materials.map(item => {
    const id = text(item.id, 1024, 1024);
    if (ids.has(id) || !Number.isSafeInteger(item.required) || item.required < 1 || item.required > 1_000_000_000 || (item.stackSize != null && (!Number.isInteger(item.stackSize) || item.stackSize < 1 || item.stackSize > 999))) throw new Error('INVALID_MATERIALS');
    ids.add(id);
    return { id, ...(item.name === undefined ? {} : {name: text(item.name, 320, 80)}), required: item.required, stackSize: item.stackSize ?? null };
  }) };
  if (new TextEncoder().encode(JSON.stringify(result)).length > 1_048_576) throw new Error('MATERIALS_TOO_LARGE');
  return result;
}
