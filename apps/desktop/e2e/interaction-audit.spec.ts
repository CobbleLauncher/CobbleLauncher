// SPDX-License-Identifier: MIT
// Synthetic native data; validates rendered entry responsibilities without running a game.
import { test, expect } from '@playwright/test';
for (const width of [560, 1280]) {
  test(`context menus and management entries ${width}`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: 800 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.route('https://**/*', route => route.abort());
    await page.addInitScript((dark) => {
      localStorage.setItem('cobble-language', 'en-US'); localStorage.setItem('cobble.appearance', dark ? 'dark' : 'light');
      const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
      target.isTauri = true; target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      const manifest = { id: 'audit-doctor', name: 'Config Doctor', version: '1.0.0', api_version: '0.2.0', publisher: 'CobbleLauncher', capabilities: ['report'], file_extensions: ['properties'], component_sha256: 'a'.repeat(64) };
      target.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => 1, invoke: async (command: string) => {
        if (command === 'plugin:event|listen') return 1;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [{ id: 'audit-game', name: 'Survival', version: '1.21.1', directory: 'fixture', status: 'installed', pinned: true }];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'list_plugins') return [{ manifest, enabled: true, problem: null }];
        if (command.startsWith('list_')) return [];
        return null;
      } };
    }, width === 560);
    await page.goto('/');
    const card = page.locator('.library-game').first();
    await expect(card.getByRole('button', { name: 'Manage Survival', exact: true })).toBeVisible();
    await expect(card.locator('.library-game-manage')).toHaveCount(0);
    await expect(card.locator('.game-menu-trigger')).toHaveCount(1);
    await card.click({ button: 'right' });
    await expect(page.getByRole('menuitem', { name: 'Manage Survival', exact: true })).toBeVisible();
    await page.screenshot({ path: info.outputPath('game-context.png'), animations: 'disabled' });
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'Extensions', exact: true }).click();
    const row = page.locator('.plugin-list li').first();
    await expect(row.getByRole('switch')).toBeVisible();
    await expect(row.getByRole('button', { name: 'Open', exact: true })).toHaveCount(0);
    await row.click({ button: 'right' });
    await expect(page.getByRole('menuitem', { name: 'Details', exact: true })).toBeVisible();
    await expect(page.getByRole('menuitem', { name: 'Uninstall', exact: true })).toBeVisible();
    await page.screenshot({ path: info.outputPath('extension-context.png'), animations: 'disabled' });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  });
}
