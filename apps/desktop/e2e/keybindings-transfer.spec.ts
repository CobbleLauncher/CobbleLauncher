// SPDX-License-Identifier: MIT
// Synthetic native boundary. No real game or options file is modified.
import { test, expect } from '@playwright/test';
test('keybinding transfer is provided by the plugin rather than native runtime settings', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 850 });
  await page.addInitScript(() => {
    localStorage.setItem('cobble-language', 'en-US');
    const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    const source = { id: 'source', name: 'Source game', status: 'installed', version: '1.21.1', directory: 'synthetic', runtime_overrides: { java: { mode: 'inherit' }, memory_mb: null } };
    target.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => 1, invoke: async (command: string) => {
      if (command === 'list_games') return [source, { ...source, id: 'target', name: 'Target game' }];
      if (command === 'list_player_profiles') return { profiles: [], active_id: null };
      if (command === 'plugin:event|listen') return 1;
      if (command === 'plugin:window|is_maximized') return false;
      if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'synthetic', custom_game_directory: false };
      if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
      if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
      if (command === 'preview_game_keybindings') return { sourceRevision: 'source', targetRevision: 'target', changes: [{ key: 'key_key.jump', before: 'key.keyboard.space', after: 'key.keyboard.a' }], skipped: 2 };
      if (command === 'apply_game_keybindings') return { backupId: 'opaque-backup', targetRevision: 'updated' };
      if (command === 'restore_game_keybindings') return 'restored';
      if (command.startsWith('list_')) return [];
      return null;
    } };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Manage Source game', exact: true }).click();
  await page.getByRole('tab', { name: 'Runtime settings', exact: true }).click();
  await expect(page.getByText('Copy keybindings to another game', { exact: true })).toHaveCount(0);
  await expect(page.locator('.keybinding-transfer')).toHaveCount(0);
});



