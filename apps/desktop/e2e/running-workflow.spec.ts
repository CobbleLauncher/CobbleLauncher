// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only: no Minecraft process, plugin execution, or public network access.
import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import type { Game } from '../src/games';
import type { CatalogManifest } from '../src/plugin-catalogs';

type Call = { command: string; args: Record<string, unknown> };
type Boundary = Window & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __runningCalls: Call[] };
const manifest = JSON.parse(readFileSync(new URL('../../../examples/command-pocket/plugin.json', import.meta.url), 'utf8')) as CatalogManifest;
const game: Game = { id: 'workflow-game', name: 'Copper Valley', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null, launch_state: 'idle', log_path: null };
const profile = { id: 'fixture-profile', kind: 'offline', name: 'Player', uuid: 'synthetic-player', note: null };
const contributions = [{ pluginId: 'command-pocket', pluginName: 'Command Pocket', settings: [], commands: [{ id: 'command-pocket.run', label: 'Build a command', workflow: 'tool_form', menus: [{ position: 'instance', when: 'instance_selected' }, { position: 'tools', when: 'always' }] }] }];

async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as Boundary).__runningCalls.filter(call => call.command === name), command);
}

test('launches once into the running workspace, opens a real tool form, and restores the running workspace from Games', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('https://**/*', route => route.abort());
  await page.addInitScript(({ manifest, profile, contributions }) => {
    const target = window as unknown as Boundary & { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => void } };
    target.isTauri = true;
    target.__runningCalls = [];
    localStorage.setItem('cobble-language', 'en-US');
    localStorage.setItem('cobble.appearance', 'light');
    let currentGame: Game = { id: 'workflow-game', name: 'Copper Valley', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null, launch_state: 'idle', log_path: null };
    let callback = 0;
    const network = { enabled: true, phase: 'online', generation: 1, attempt: 1, retry_at: null, profile: null, services: [], last_error: null, relay_available: false, session_route: null, checks: [] };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__runningCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [currentGame];
        if (command === 'list_player_profiles') return { profiles: [{ ...profile }], active_id: profile.id };
        if (command === 'list_tasks') return [];
        if (command === 'get_automatic_world_backup') return { preference: 'disabled', last_backup: null, last_error: null };
        if (command === 'mesh_network_snapshot' || command === 'mesh_network_start') return network;
        if (command === 'peek_pending_schematic' || command === 'peek_pending_launch') return null;
        if (command === 'start_game') {
          currentGame = { ...currentGame, launch_state: 'running', last_launched_at: 1_798_197_600 };
          return currentGame;
        }
        if (command === 'list_extension_contributions') return contributions;
        if (command === 'resolve_extension_command') return { pluginId: args.pluginId, commandId: args.commandId, label: 'Build a command', workflow: 'tool_form', instanceId: (args.context as { instanceId?: string } | undefined)?.instanceId };
        if (command === 'list_plugins') return [{ manifest, enabled: true }];
        if (command === 'run_plugin_tool_form') return { title: 'Generated command', blocks: [{ label: 'Command', text: '/tp @s ~ ~ ~' }] };
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'synthetic', custom_game_directory: false };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_game_java_runtimes') return { runtimes: [], requiredMajor: 21, recommendedId: null };
        if (command.startsWith('list_')) return [];
        return null;
      },
    };
  }, { manifest, profile, contributions });

  await page.setViewportSize({ width: 1100, height: 850 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.getByRole('button', { name: 'Launch Copper Valley', exact: true }).click();
  await expect(page.locator('.running-workspace')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Game is running' })).toBeVisible();
  await page.evaluate(async () => { await document.fonts.ready; });
  await page.screenshot({ path: test.info().outputPath('running-workspace.png'), fullPage: true, animations: 'disabled' });
  expect(await calls(page, 'start_game')).toEqual([{ command: 'start_game', args: { id: game.id } }]);
  await expect(page.getByRole('button', { name: 'Launch again', exact: true })).toHaveCount(0);

  await page.getByRole('button', { name: 'Build a command', exact: true }).click();
  await expect(page.locator('.tool-form-panel')).toBeVisible();
  await page.evaluate(async () => { await document.fonts.ready; });
  await page.screenshot({ path: test.info().outputPath('running-tool-form.png'), fullPage: true, animations: 'disabled' });
  expect((await calls(page, 'resolve_extension_command')).at(-1)?.args).toMatchObject({ pluginId: 'command-pocket', commandId: 'command-pocket.run', context: { instanceId: game.id } });
  await page.getByRole('button', { name: 'Generate', exact: true }).click();
  await expect(page.locator('.tool-form-output textarea')).toHaveValue('/tp @s ~ ~ ~');
  expect((await calls(page, 'run_plugin_tool_form')).at(-1)?.args).toMatchObject({ pluginId: 'command-pocket', input: { version: '1.21.1', template: 'teleport' } });
  const usage = await page.evaluate(() => JSON.parse(localStorage.getItem('cobble.tools.usage.v1') ?? '{}'));
  expect(usage.entries).toEqual([expect.objectContaining({ pluginId: 'command-pocket', commandId: 'command-pocket.run' })]);

  await page.getByRole('button', { name: 'Back to Copper Valley', exact: true }).click();
  await expect(page.locator('.running-workspace')).toBeVisible();
  await page.getByRole('button', { name: 'View settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Runtime settings', exact: true }).click();
  await expect(page.locator('fieldset.runtime-write-controls')).toHaveAttribute('disabled', '');
  await expect(page.locator('fieldset.runtime-write-controls button').first()).toBeDisabled();
  await page.getByRole('button', { name: 'Games', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Manage Copper Valley', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Manage Copper Valley', exact: true }).click();
  await expect(page.locator('.running-workspace')).toBeVisible();
  expect(await calls(page, 'start_game')).toHaveLength(1);
  expect(errors).toEqual([]);
});
