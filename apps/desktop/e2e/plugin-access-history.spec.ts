// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only; no native plugin execution or real access history.
import { expect, test, type Page } from '@playwright/test';

type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & { __calls: Call[] };
const providerId = 'mcs-provider';
const detailsId = 'config-doctor';
const providerManifest = {
  id: providerId,
  name: 'MCS Blueprint Provider',
  version: '1.0.0',
  api_version: '0.2.0',
  publisher: 'CobbleLauncher',
  capabilities: ['blueprint-provider'],
  file_extensions: [],
  component_sha256: 'a'.repeat(64),
  provider_origins: ['https://www.mcschematic.top'],
  permissions: [{ permission: 'launcher:provider/https/get', origins: ['https://www.mcschematic.top'] }],
};
const detailsManifest = {
  id: detailsId,
  name: 'Config Doctor',
  version: '0.2.0',
  api_version: '0.1.0',
  publisher: 'CobbleLauncher',
  capabilities: ['report'],
  file_extensions: ['json', 'properties'],
  component_sha256: 'b'.repeat(64),
  permissions: [
    { permission: 'launcher:resource/selected-folder/read', extensions: ['json', 'properties'] },
    { permission: 'launcher:ui/webview' },
    { permission: 'launcher:game-config/read' },
    { permission: 'launcher:game-keybindings/read' },
    { permission: 'launcher:game-keybindings/write' },
  ],
  frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'c'.repeat(64) },
};
const provider = { manifest: providerManifest, enabled: true, problem: null };
const detailsPlugin = { manifest: detailsManifest, enabled: true, problem: null };
const records = [
  { timestamp_ms: 1792700000000, plugin_id: detailsId, capability: 'game-keybindings-write', action: 'keybindings.apply', outcome: 'denied', target: 'game:fixture-game' },
  { timestamp_ms: 1792700001000, plugin_id: detailsId, capability: 'game-keybindings-write', action: 'keybindings.restore', outcome: 'completed', target: 'game:fixture-game' },
];

async function setup(page: Page) {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.route('https://**/*', route => route.abort());
  await page.addInitScript(({ provider, detailsPlugin, records }) => {
    localStorage.setItem('cobble-language', 'en-US');
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true;
    target.__calls = [];
    let callbackId = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callbackId,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') return callbackId;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_plugins') return [provider, detailsPlugin];
        if (command === 'get_plugin_details') return args.id === detailsPlugin.manifest.id
          ? { plugin: detailsPlugin, signature: { status: 'unsigned' }, provides: [], dependencies: [] }
          : { plugin: provider, signature: { status: 'unsigned' }, provides: [], dependencies: [] };
        if (command === 'plugin_access_log') return args.pluginId === detailsPlugin.manifest.id ? records : [];
        if (command.startsWith('list_')) return [];
        return null;
      },
    };
  }, { provider, detailsPlugin, records });
  await page.goto('/');
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  const providerRow = page.locator('.plugin-list li').filter({ hasText: providerManifest.name });
  const detailsRow = page.locator('.plugin-list li').filter({ hasText: detailsManifest.name });
  await expect(providerRow.locator('.plugin-permission-badge', { hasText: 'Network' })).toBeVisible();
  await expect(detailsRow.locator('.plugin-permission-badge', { hasText: 'Local files' })).toBeVisible();
  await expect(detailsRow.locator('.plugin-permission-badge', { hasText: 'Can write' })).toBeVisible();
  await detailsRow.getByRole('button', { name: `Details: ${detailsManifest.name}`, exact: true }).click();
  await expect(page.locator('.plugin-details-permissions')).toBeVisible();
}

async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as Mock).__calls.filter(call => call.command === name), command);
}

test('permission badges and selected-plugin access history render without eager loading or overflow', async ({ page }, testInfo) => {
  await setup(page);
  await expect(page.locator('.plugin-details-summary .plugin-permission-badge')).toHaveCount(2);
  expect((await calls(page, 'get_plugin_details')).map(call => call.args)).toEqual([{ id: detailsId, locale: 'en' }]);
  expect(await calls(page, 'plugin_access_log')).toHaveLength(0);
  await page.getByRole('button', { name: 'Recent access', exact: true }).click();
  await expect(page.getByText('Write keybindings', { exact: true })).toBeVisible();
  await expect(page.getByText('Denied', { exact: true })).toBeVisible();
  await expect(page.getByText('Restore keybindings', { exact: true })).toBeVisible();
  await expect(page.getByText('Completed', { exact: true })).toBeVisible();
  expect((await calls(page, 'plugin_access_log')).map(call => call.args)).toEqual([{ pluginId: detailsId }]);
  for (const width of [1280, 560]) {
    await page.setViewportSize({ width, height: 900 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath(`plugin-access-history-${width}.png`), fullPage: true, animations: 'disabled' });
  }
});
