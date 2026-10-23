// SPDX-License-Identifier: MIT
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';

it('declares desktop commands and grants the frontend broker only to plugin views', () => {
  const read = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8');
  const main = read('../src-tauri/src/main.rs');
  const handler = main.split('tauri::generate_handler![')[1]?.split(']')[0];
  expect(handler).toBeTruthy();
  const declarations = read('../src-tauri/build.rs');
  const { permissions } = JSON.parse(read('../src-tauri/capabilities/main.json')) as { permissions: string[] };
  const pluginView = JSON.parse(read('../src-tauri/capabilities/plugin-view.json')) as { permissions: string[]; webviews: string[] };
  expect(pluginView.permissions).toEqual(['allow-frontend-call']);
  expect(pluginView.webviews).toEqual(['plugin-ui-*']);
  expect(permissions).not.toContain('allow-frontend-call');
  for (const entry of handler!.split(',').map(value => value.trim()).filter(Boolean)) {
    const command = entry.split('::').at(-1)!;
    expect(declarations, command).toContain(`"${command}"`);
    if (command === 'frontend_call') continue;
    expect(permissions, command).toContain(`allow-${command.replaceAll('_', '-')}`);
  }
});
