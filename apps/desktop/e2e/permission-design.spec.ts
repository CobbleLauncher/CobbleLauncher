// SPDX-License-Identifier: MIT
// Synthetic Tauri responses for visual review; no native plugin, catalog or network execution.
import { expect, test, type Page } from '@playwright/test';
import type { CatalogManifest } from '../src/plugin-catalogs';

type Call = { command: string; args: Record<string, unknown> };
type Boundary = Window & { __permissionCalls: Call[]; isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
const previous: CatalogManifest = {
  id: 'fixture.config-companion', name: 'Config Companion', publisher: 'Example Tools',
  version: '1.0.0', api_version: '0.2.0', capabilities: ['report'],
  file_extensions: ['json', 'properties'], component_sha256: 'a'.repeat(64), permissions: [],
};
const next: CatalogManifest = {
  ...previous, version: '1.1.0', component_sha256: 'b'.repeat(64),
  permissions: [
    { permission: 'launcher:resource/selected-folder/read', extensions: ['json', 'properties'] },
    { permission: 'launcher:provider/https/get', origins: ['https://catalog.example.test'] },
    { permission: 'launcher:game-keybindings/write' },
    { permission: 'launcher:composer/context/selected-session' },
  ],
};
async function setup(page: Page, theme: 'light' | 'dark') {
  await page.route('https://**/*', route => route.abort());
  await page.addInitScript(({ previous, next, theme }) => {
    localStorage.setItem('cobble-language', 'en-US');
    localStorage.setItem('cobble.appearance', theme);
    const target = window as unknown as Boundary;
    target.isTauri = true; target.__permissionCalls = [];
    let callbackId = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callbackId,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__permissionCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callbackId;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_plugins') return [{ manifest: previous, enabled: true }];
        if (command === 'list_bundled_plugins') return [next];
        if (command === 'preview_bundled_plugin') return next;
        if (command === 'get_pending_plugin_signature') return { status: 'unsigned' };
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command.startsWith('list_')) return [];
        return null;
      },
    };
  }, { previous, next, theme });
  await page.goto('/');
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  await expect(page.locator('.plugin-updates')).toBeVisible();
}

for (const width of [1100, 560]) {
  for (const theme of ['light', 'dark'] as const) {
    test(`permission consent design ${width} ${theme}`, async ({ page }, testInfo) => {
      await page.setViewportSize({ width, height: 1000 });
      await setup(page, theme);
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.screenshot({ path: testInfo.outputPath(`updates-${width}-${theme}.png`), fullPage: true });
      await page.locator('.plugin-updates').getByRole('button', { name: 'Review update', exact: true }).click();
      const approval = page.locator('.plugin-approval');
      const checks = approval.getByRole('checkbox');
      await expect(checks).toHaveCount(4);
      for (const check of await checks.all()) await expect(check).not.toBeChecked();
      await expect(approval.locator('.sensitive-permissions')).toContainText('https://catalog.example.test');
      await expect(approval.locator('.plugin-permission-expansion')).toContainText('selected game');
      const approve = approval.locator('.primary-button');
      await expect(approve).toBeDisabled();
      await checks.first().focus();
      await expect(checks.first()).toBeFocused();
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.screenshot({ path: testInfo.outputPath(`consent-focus-${width}-${theme}.png`), fullPage: true });
      await page.keyboard.press('Space');
      await expect(checks.first()).toBeChecked();
      await expect(approve).toBeDisabled();
      for (const check of (await checks.all()).slice(1)) await check.check();
      await expect(approve).toBeEnabled();
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.screenshot({ path: testInfo.outputPath(`consent-checked-${width}-${theme}.png`), fullPage: true });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      expect(await page.evaluate(() => (window as unknown as Boundary).__permissionCalls.filter(call => ['install_plugin', 'update_plugin'].includes(call.command)))).toEqual([]);
    });
  }
}
