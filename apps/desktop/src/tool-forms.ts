// SPDX-License-Identifier: MIT
import type { ToolFormManifest } from './plugin-catalogs';
export { schemaUiRules as toolFormUiSchema } from './extensions/forms/ui-schema-rules';
export type ToolFormDefinition = ToolFormManifest;
export type ToolFormOutput = { title: string; blocks: { label: string | null; text: string }[] };
export function toolFormDefaults(form: ToolFormDefinition, contextVersion?: string): Record<string, unknown> {
  const values: Record<string, unknown> = {};
  for (const [key, schema] of Object.entries(form.schema.properties ?? {})) {
    if (typeof schema !== 'object' || schema === null) continue;
    values[key] = schema.default;
  }
  if (contextVersion) for (const key of ['minecraftVersion', 'version']) {
    const schema = form.schema.properties?.[key];
    if (typeof schema === 'object' && schema !== null && schema.enum?.includes(contextVersion)) { values[key] = contextVersion; break; }
  }
  return values;
}
