// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary; this test changes no real game or Mod files.
import { expect, test } from '@playwright/test';
import type { Game, LocalMod } from '../src/games';
test('extracted Mods workflow preserves batch locks, disabled names, confirmations and English labels', async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    const target = window as unknown as Window & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __modsCalls: { command: string; args: Record<string, unknown> }[]; __finishModMutation: () => void };
    target.isTauri = true; target.__modsCalls = []; localStorage.setItem('cobble-language', 'en-US');
    const game: Game = { id: 'mods-game', name: 'Mod game', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null };
    let mods: LocalMod[] = [{ file_name: 'alpha.jar', name: 'Alpha', version: '1', authors: ['A'], loader: 'fabric', enabled: true, description: 'Original fixture' }, { file_name: 'beta.jar', name: 'Beta', version: '2', authors: ['B'], loader: 'fabric', enabled: true, description: 'Another original fixture' }];
    let callback = 0; target.__finishModMutation = () => {};
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__modsCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [game];
        if (command === 'list_tasks' || command === 'list_managed_game_mods' || command === 'list_extension_contributions') return [];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'player', kind: 'offline', name: 'Tester', uuid: 'synthetic-player', note: null }], active_id: 'player' };
        if (command === 'get_curseforge_availability') return { available: false, reason: null };
        if (command === 'list_game_mods') return structuredClone(mods);
        if (command === 'set_game_mod_enabled') {
          await new Promise<void>(resolve => { target.__finishModMutation = resolve; });
          mods = mods.map(mod => mod.file_name === args.fileName ? { ...mod, enabled: Boolean(args.enabled), file_name: args.enabled ? mod.file_name.replace(/\.disabled$/, '') : `${mod.file_name}.disabled` } : mod);
          return structuredClone(mods);
        }
        if (command === 'remove_game_mod') { mods = mods.filter(mod => mod.file_name !== args.fileName); return structuredClone(mods); }
        if (command === 'import_game_mods') return null;
        return null;
      },
    };
  });
  await page.setViewportSize({ width: 560, height: 800 }); await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/'); await page.getByRole('button', { name: 'Manage Mod game', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Mod game', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open game folder', exact: true })).toBeVisible();
  await expect(page.getByRole('list', { name: 'Local Mods', exact: true })).toContainText('Alpha');
  await page.getByRole('searchbox', { name: 'Search local Mods', exact: true }).fill('Beta');
  await page.getByRole('checkbox', { name: 'Select all visible Mods', exact: true }).check();
  await page.getByRole('button', { name: 'Disable selected', exact: true }).click();
  await expect(page.locator('.play-button')).toBeDisabled();
  await expect(page.getByRole('tab', { name: 'Worlds', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Back to game library', exact: true })).toBeDisabled();
  await page.evaluate(() => (window as unknown as { __finishModMutation: () => void }).__finishModMutation());
  await expect(page.getByText('beta.jar.disabled', { exact: true })).toBeVisible();
  await expect(page.getByRole('switch', { name: 'Enable Beta', exact: true })).toHaveAttribute('aria-checked', 'false');
  await expect(page.locator('.play-button')).toBeEnabled();
  await page.getByRole('searchbox', { name: 'Search local Mods', exact: true }).clear();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect(page.locator('.details-summary')).toBeInViewport();
  await page.screenshot({ path: testInfo.outputPath('game-mods-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: 'Import Mods', exact: true }).click();
  await expect(page.locator('.mod-row')).toHaveCount(2);
  await page.getByRole('checkbox', { name: 'Select all visible Mods', exact: true }).check();
  await page.getByRole('button', { name: 'Remove selected', exact: true }).click();
  expect(await page.evaluate(() => (window as unknown as { __modsCalls: { command: string }[] }).__modsCalls.some(call => call.command === 'remove_game_mod'))).toBe(false);
  await page.getByRole('button', { name: 'Confirm removal', exact: true }).click();
  await expect(page.locator('.mod-row')).toHaveCount(0);
  await expect(page.getByText('No Mods yet. Import local .jar files to start.', { exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { __modsCalls: { command: string; args: unknown }[] }).__modsCalls.filter(call => call.command === 'remove_game_mod').map(call => call.args))).toEqual([{ id: 'mods-game', fileName: 'alpha.jar' }, { id: 'mods-game', fileName: 'beta.jar.disabled' }]);
});
