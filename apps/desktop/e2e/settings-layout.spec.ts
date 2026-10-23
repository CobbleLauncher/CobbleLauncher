// SPDX-License-Identifier: MIT
// Synthetic native boundary; no real preference files or Java runtimes are changed.
import { expect, test } from '@playwright/test';
test('settings keeps drafts explicit, groups paths with actions, and fits narrow windows', async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    const target = window as unknown as Window & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __settingsCalls: { command: string; args: Record<string, unknown> }[] };
    target.isTauri = true; target.__settingsCalls = []; localStorage.setItem('cobble-language', 'en-US');
    let preferences = { memory_mb: 4096, game_directory: 'C:\\Games\\Minecraft', custom_game_directory: true };
    const game = { id: 'settings-game', name: 'Settings game', version: '1.21.1', loader_kind: null, loader_version: null, directory: 'synthetic', status: 'installed', progress: null, error: null };
    let callback = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__settingsCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [game];
        if (['list_tasks', 'list_plugins', 'list_plugin_settings', 'list_extension_contributions', 'list_game_packs', 'list_game_mods', 'list_managed_game_mods', 'list_game_resource_packs', 'list_game_shader_packs', 'list_game_worlds'].includes(command)) return [];
        if (command === 'list_game_java_runtimes') return { runtimes: [], requiredMajor: 21, recommendedId: null };
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
        if (command === 'get_launcher_preferences') return preferences;
        if (command === 'get_download_tuning') return { max_concurrent: 12, per_origin: 4 };
        if (command === 'get_public_cache_stats') return { disk_bytes: 1048576 };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
        if (command === 'set_default_game_directory') { preferences = { ...preferences, game_directory: String(args.path) }; return preferences; }
        return null;
      },
    };
  });
  await page.setViewportSize({ width: 1000, height: 850 }); await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/'); await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Language', exact: true }).click();
  await expect(page.getByRole('listbox')).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('settings-language-1000.png'), fullPage: true, animations: 'disabled' });
  await page.keyboard.press('Escape');
  await page.getByRole('tab', { name: 'Downloads & storage', exact: true }).click();
  await page.getByRole('textbox', { name: 'New game directory', exact: true }).fill('D:\\New Games');
  expect(await page.evaluate(() => (window as unknown as { __settingsCalls: { command: string }[] }).__settingsCalls.some(call => call.command === 'set_default_game_directory'))).toBe(false);
  const directory = page.locator('.settings-row').filter({ has: page.locator('#default-game-directory') });
  await directory.getByRole('button', { name: 'Apply', exact: true }).click();
  await expect(directory.locator('.settings-actions')).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: 'New game directory', exact: true })).toHaveValue('D:\\New Games');
  await page.screenshot({ path: testInfo.outputPath('settings-downloads-1000.png'), fullPage: true, animations: 'disabled' });
  await page.setViewportSize({ width: 560, height: 800 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('settings-downloads-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('tab', { name: 'Game defaults', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Browse for Java', exact: true })).not.toBeVisible();
  await page.getByRole('spinbutton', { name: 'Default maximum memory (MiB)', exact: true }).fill('8192');
  const memory = page.locator('.settings-row').filter({ has: page.locator('#default-memory') });
  await memory.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.getByRole('spinbutton', { name: 'Default maximum memory (MiB)', exact: true })).toHaveValue('4096');
  await page.locator('.advanced-runtime-settings > .settings-disclosure-trigger').click();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('settings-runtime-560.png'), fullPage: true, animations: 'disabled' });
  await page.setViewportSize({ width: 1000, height: 850 });
  await page.screenshot({ path: testInfo.outputPath('settings-runtime-1000.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: 'Games', exact: true }).click();
  await page.getByRole('button', { name: 'Manage Settings game', exact: true }).click();
  await page.getByRole('tab', { name: 'Runtime settings', exact: true }).click();
  await page.locator('.game-appearance-panel>.settings-disclosure-trigger').click();
  await expect(page.getByRole('combobox', { name: 'Java', exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('instance-settings-1000.png'), fullPage: true, animations: 'disabled' });
  await page.setViewportSize({ width: 560, height: 800 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('instance-settings-560.png'), fullPage: true, animations: 'disabled' });
});
