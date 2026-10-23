// SPDX-License-Identifier: MIT
// Synthetic native boundary: verifies composition and navigation, not native installation.
import { expect, test } from '@playwright/test';
test('top navigation preserves settings drafts in a narrow English layout', async ({ page }, info) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true;
    localStorage.setItem('cobble-language', 'en-US');
    let callback = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
      invoke: async (command: string) => {
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (['list_games', 'list_tasks', 'list_plugins', 'list_extension_contributions', 'list_plugin_catalogs', 'list_bundled_plugins', 'list_blueprints'].includes(command)) return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'C:/Games', custom_game_directory: false };
        if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
        if (command === 'get_download_tuning') return { max_concurrent: 12, per_origin: 4 };
        if (command === 'get_public_cache_stats') return { disk_bytes: 0 };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
        return null;
      },
    };
  });
  await page.setViewportSize({ width: 560, height: 760 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Game defaults', exact: true }).click();
  await page.getByLabel('Default maximum memory (MiB)', { exact: true }).fill('8192');
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Keep editing', exact: true }).click();
  await expect(page.getByLabel('Default maximum memory (MiB)', { exact: true })).toHaveValue('8192');
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  await page.getByRole('button', { name: 'Discard changes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Extensions', exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Sources', exact: true }).click();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('extensions-en-560.png'), animations: 'disabled' });
  await page.getByRole('button', { name: 'Tools', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Tools', exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath('tools-en-560.png'), animations: 'disabled' });
  expect(errors).toEqual([]);
});
