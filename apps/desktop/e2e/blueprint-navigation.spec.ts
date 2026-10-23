// SPDX-License-Identifier: MIT
// Synthetic native boundary; real Vue navigation/layout, no provider or native success claims.
import { expect, test } from '@playwright/test';

test('blueprints opens the saved list directly and offers an optional online catalog', async ({ page }, info) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    localStorage.setItem('cobble-language', 'en-US');
    const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true;
    let callback = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
      invoke: async (command: string) => {
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'get_blueprint_viewer_choices') return { viewer_id: 'synthetic-viewer', viewers: [{ id: 'synthetic-viewer', name: 'Fixture Viewer', file_extensions: ['schem', 'litematic'] }], disabled_viewers: [], interface: { id: 'launcher:viewer', version: '^1.0.0', implementations: [{ plugin_id: 'synthetic-viewer', name: 'Fixture Viewer', version: '1.0.0' }], candidates: [] } };
        if (command === 'list_blueprints' && localStorage.getItem('blueprint-fixture-empty') === 'yes') return [];
        if (command === 'list_blueprints') return [{ id: 'workshop', name: 'Workshop', pluginId: 'viewer', format: 'schem' }, { id: 'mountain', name: 'Mountain base', pluginId: 'viewer', format: 'litematic' }];
        if (['list_games', 'list_tasks', 'list_plugins', 'list_extension_contributions', 'list_plugin_catalogs', 'list_bundled_plugins'].includes(command)) return [];
        if (command === 'get_blueprint_provider_availability') return { available: false, providers: [], provider_id: null, viewer_available: false, interface: { id: 'launcher:blueprint-provider', version: '^0.2.0', implementations: [], candidates: localStorage.getItem('blueprint-fixture-candidates') === 'yes' ? [{ source_id: 'source-a', plugin_id: 'community-catalog', name: 'Community catalog', version: '1.4.0' }, { source_id: 'source-b', plugin_id: 'another-catalog', name: 'Another catalog', version: '2.0.0' }] : [] } };
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'C:/Games', custom_game_directory: false };
        if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
        if (command === 'get_download_tuning') return { max_concurrent: 12, per_origin: 4 };
        if (command === 'get_public_cache_stats') return { disk_bytes: 0 };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        return null;
      },
    };
  });
  await page.setViewportSize({ width: 1100, height: 760 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Tools', exact: true }).click();
  await page.getByRole('button', { name: /Blueprint library Browse/ }).click();
  await expect(page.locator('.blueprint-open').filter({ hasText: 'Workshop' })).toBeVisible();
  await expect(page.getByRole('searchbox', { name: 'Search saved blueprints' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'All tools', exact: true })).toHaveCount(0);
  await page.screenshot({ path: info.outputPath('blueprint-library-en-1100.png'), animations: 'disabled' });
  await page.getByRole('searchbox', { name: 'Search saved blueprints' }).fill('mountain');
  await expect(page.locator('.blueprint-open').filter({ hasText: 'Workshop' })).toHaveCount(0);
  await page.getByRole('tab', { name: 'Online blueprints', exact: true }).click();
  await expect(page.getByText('Install and enable a blueprint provider first.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Manage plugins', exact: true })).toBeVisible();
  await page.setViewportSize({ width: 560, height: 760 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('blueprint-online-unavailable-en-560.png'), animations: 'disabled' });
  await page.evaluate(() => localStorage.setItem('blueprint-fixture-candidates', 'yes'));
  await page.getByRole('button', { name: 'Retry', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Community catalog 1.4.0', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Another catalog 2.0.0', exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath('blueprint-provider-candidates-en-560.png'), animations: 'disabled' });
  await page.evaluate(() => localStorage.setItem('blueprint-fixture-empty', 'yes'));
  await page.getByRole('button', { name: 'Games', exact: true }).click();
  await page.getByRole('button', { name: 'Tools', exact: true }).click();
  await page.getByRole('button', { name: /Blueprint library Browse/ }).click();
  await expect(page.getByText('No saved blueprints yet.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open .schem / .litematic', exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath('blueprint-empty-en-560.png'), animations: 'disabled' });
  expect(errors).toEqual([]);
});
