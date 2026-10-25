// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only: no Minecraft process, plugin execution, or public network access.
import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import type { Game } from '../src/games';
import type { CatalogManifest } from '../src/plugin-catalogs';

type Call = { command: string; args: Record<string, unknown> };
type Boundary = Window & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __groupCalls: Call[] };
const seedManifest = JSON.parse(readFileSync(new URL('../../../examples/seed-compass/plugin.json', import.meta.url), 'utf8')) as CatalogManifest;
const game: Game = { id: 'group-entry-game', name: 'Copper Valley', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null, launch_state: 'running', log_path: null };
const profile = { id: 'fixture-profile', kind: 'offline', name: 'Player', uuid: 'synthetic-player', note: null };

async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as Boundary).__groupCalls.filter(call => call.command === name), command);
}

test('returns from group setup to the running workspace and finds bundled Seed Compass without installing it', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('https://**/*', route => route.abort());
  await page.addInitScript(({ seedManifest, profile, game }) => {
    const target = window as unknown as Boundary & { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => void } };
    target.isTauri = true;
    target.__groupCalls = [];
    localStorage.setItem('cobble-language', 'en-US');
    localStorage.setItem('cobble.appearance', 'light');
    let callback = 0;
    const network = { enabled: false, phase: 'offline', generation: 1, attempt: 0, retry_at: null, profile: null, services: [], last_error: null, relay_available: false, session_route: null, checks: [] };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__groupCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [{ ...game }];
        if (command === 'list_player_profiles') return { profiles: [{ ...profile }], active_id: profile.id };
        if (command === 'list_tasks' || command === 'list_plugins' || command === 'list_extension_contributions' || command === 'list_plugin_catalogs') return [];
        if (command === 'list_bundled_plugins') return [seedManifest];
        if (command === 'mesh_network_snapshot' || command === 'mesh_network_start') return network;
        if (command === 'mesh_group_snapshot') return { generation: 0, group: null };
        if (command === 'mesh_host_options') return { game_id: game.id, lan: { state: 'not_open', port: null }, relay: { address: null, source: null, state: 'unconfigured' } };
        if (command === 'mesh_identity_state') return { profile: null, mode: 'temporary', active: false };
        if (command === 'mesh_app_snapshot') return null;
        if (command === 'get_automatic_world_backup') return { preference: 'disabled', last_backup: null, last_error: null };
        if (command === 'peek_pending_schematic' || command === 'peek_pending_launch') return null;
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'synthetic', custom_game_directory: false };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_game_java_runtimes') return { runtimes: [], requiredMajor: 21, recommendedId: null };
        if (command.startsWith('list_')) return [];
        return null;
      },
    };
  }, { seedManifest, profile, game });

  await page.setViewportSize({ width: 1100, height: 850 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.getByRole('button', { name: 'Manage Copper Valley', exact: true }).click();
  await expect(page.locator('.running-workspace')).toBeVisible();
  await page.getByRole('button', { name: 'Play with friends', exact: true }).click();
  await expect(page.locator('.mesh-group-panel')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Play together', exact: true })).toBeVisible();
  await page.evaluate(async () => { await document.fonts.ready; });
  await page.screenshot({ path: test.info().outputPath('mesh-group-entry.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: 'Back to Copper Valley', exact: true }).click();
  await expect(page.locator('.running-workspace')).toBeVisible();
  await expect(page.locator('.mesh-group-panel')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Back to Copper Valley', exact: true })).toHaveCount(0);

  await page.getByRole('button', { name: 'Seed Compass', exact: true }).click();
  await expect(page.locator('.plugins-panel')).toBeVisible();
  await expect(page.getByRole('tab', { name: 'Discover', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.plugins-panel input[type="search"]')).toHaveValue('seed-compass');
  await expect(page.getByText('Seed Compass', { exact: true })).toBeVisible();
  expect(await calls(page, 'install_plugin')).toEqual([]);
  expect(await calls(page, 'preview_bundled_plugin')).toEqual([]);
  await page.evaluate(async () => { await document.fonts.ready; });
  await page.screenshot({ path: test.info().outputPath('seed-compass-discovery.png'), fullPage: true, animations: 'disabled' });

  await page.getByRole('button', { name: 'Back to Copper Valley', exact: true }).click();
  await expect(page.locator('.running-workspace')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Back to Copper Valley', exact: true })).toHaveCount(0);
  expect(await calls(page, 'start_game')).toEqual([]);
  expect(errors).toEqual([]);
});
