// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary for populated catalog review; no native tool execution.
import { expect, test } from '@playwright/test';
import type { CatalogManifest } from '../src/plugin-catalogs';

type Call = { command: string; args: Record<string, unknown> };
type Boundary = Window & { __toolDesignCalls: Call[]; isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
const tools: CatalogManifest[] = [
  { id: 'fixture.mesh-preview', name: 'Mesh Preview', description: 'Inspect models and schematic geometry.', tags: ['creative', 'preview'] },
  { id: 'fixture.crash-doctor', name: 'Crash Doctor', description: 'Find causes in recent logs and crash reports.', tags: ['diagnostics', 'logs'] },
  { id: 'fixture.world-inventory', name: 'World Inventory', description: 'Inspect saves and their dimensions.', tags: ['worlds', 'data'] },
  { id: 'fixture.runtime-check', name: 'Runtime Check', description: 'Inspect Java and launch settings.', tags: ['runtime', 'java'] },
  { id: 'fixture.config-doctor', name: 'Config Doctor', description: 'Review game configuration files.', tags: ['config', 'diagnostics'] },
].map(tool => ({ ...tool, publisher: 'Example Tools', version: '1.0.0', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['json'], component_sha256: 'a'.repeat(64), permissions: [] }));

test('populated tool catalog uses semantic accents and preserves game context by keyboard', async ({ page }, info) => {
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.route('https://**/*', route => route.abort());
  await page.addInitScript(({ tools }) => {
    localStorage.setItem('cobble-language', 'en-US');
    if (!localStorage.getItem('cobble.appearance')) localStorage.setItem('cobble.appearance', 'light');
    const target = window as unknown as Boundary;
    target.isTauri = true; target.__toolDesignCalls = [];
    let callback = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__toolDesignCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [
          { id: 'older-game', name: 'Earlier world', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, last_launched_at: 1 },
          { id: 'recent-game', name: 'Recent world', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, last_launched_at: 20 },
        ];
        if (command === 'list_plugins') return tools.map(manifest => ({ manifest, enabled: true }));
        if (command === 'list_extension_contributions') return tools.map(manifest => ({ pluginId: manifest.id, pluginName: manifest.name, settings: [], commands: [{ id: 'inspect', label: manifest.name, workflow: 'report', menus: [{ position: 'tools', when: 'always' }] }] }));
        if (command === 'resolve_extension_command') return { pluginId: args.pluginId, commandId: args.commandId, label: tools.find(tool => tool.id === args.pluginId)?.name, workflow: 'report', instanceId: (args.context as { instanceId?: string } | undefined)?.instanceId };
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command.startsWith('list_')) return [];
        return null;
      },
    };
  }, { tools });
  for (const [width, theme] of [[1100, 'light'], [560, 'dark']] as const) {
    await page.setViewportSize({ width, height: 1000 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/');
    await page.evaluate(theme => localStorage.setItem('cobble.appearance', theme), theme);
    await page.reload();
    await page.getByRole('button', { name: 'Tools', exact: true }).click();
    await expect(page.locator('.tool-list > li')).toHaveCount(6);
    const colors = await page.locator('.tool-list > li').evaluateAll(entries => [...new Set(entries.map(entry => getComputedStyle(entry).getPropertyValue('--tool-tone').trim()))]);
    expect(colors.length).toBeGreaterThanOrEqual(4);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const crash = page.getByRole('button', { name: 'Crash Doctor', exact: true });
    await crash.focus(); await expect(crash).toBeFocused();
    await page.screenshot({ path: info.outputPath(`tools-populated-${width}-${theme}.png`), fullPage: true, animations: 'disabled' });
    await page.keyboard.press('Enter');
    await expect(page.locator('.plugin-report-panel')).toBeVisible();
    const resolved = await page.evaluate(() => (window as unknown as Boundary).__toolDesignCalls.find(call => call.command === 'resolve_extension_command'));
    expect(resolved?.args).toMatchObject({ position: 'tools', context: { instanceId: 'recent-game' } });
  }
  expect(errors).toEqual([]);
});
