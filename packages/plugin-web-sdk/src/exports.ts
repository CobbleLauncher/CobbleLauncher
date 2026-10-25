// SPDX-License-Identifier: MIT
export type TextExport = {name: string; text: string};
export function validateTextExport(value: TextExport): TextExport {
  if (!value || typeof value.name !== 'string' || !value.name.trim() || [...value.name].length > 96 || /[\u0000-\u001f\u007f<>:"/\\|?*]/u.test(value.name) || /^[. ]|[. ]$/u.test(value.name)) throw new Error('PLUGIN_EXPORT_NAME_INVALID');
  if (typeof value.text !== 'string' || value.text.includes('\0') || new TextEncoder().encode(value.text).length > 1_048_576) throw new Error('PLUGIN_EXPORT_TEXT_INVALID');
  return {name: value.name, text: value.text};
}
export function createExports(transport: (request: {method: 'exports.text'; params: TextExport}) => Promise<unknown>) {
  return Object.freeze({text: (input: TextExport) => transport({method: 'exports.text', params: validateTextExport(input)}) as Promise<{saved: boolean}>});
}
