// SPDX-License-Identifier: MIT
// Synthetic native boundary only; no Modrinth requests or game files are used.
import { test, expect, type Page } from '@playwright/test';
import type { Game, LocalMod } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & {
  __calls: Call[];
  __failRecognition: boolean;
  __finishRecognition: () => void;
  __blockNextRecognition: () => void;
};
const game: Game = { id: 'forest', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
const mods: LocalMod[] = [
  { file_name: 'map.jar', name: 'Forest Map', version: '1.0', authors: ['Forest'], loader: 'fabric', enabled: true, description: 'Map', pack_owned: true },
  { file_name: 'unknown.jar', name: 'Unknown Mod', version: null, authors: [], loader: 'fabric', enabled: true, description: null, pack_owned: false },
];
const updates = [{ project_id: 'map-project', name: 'Forest Map', current_version: '1.0', version_number: '2.0', version_id: 'map-v2', filename: 'map.jar', enabled: true, pack_owned: true }];

async function setup(page: Page) {
  await page.addInitScript(({ game, mods, updates }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__failRecognition = false;
    const localMods = [...mods];
    let recognitionBlocked = false;
    let finishRecognition: () => void = () => {};
    target.__finishRecognition = () => finishRecognition();
    target.__blockNextRecognition = () => { recognitionBlocked = true; };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') return 1;
        if (command === 'list_games') return [game];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Player', uuid: 'synthetic', note: null }], active_id: 'test-player' };
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_game_mods') return localMods;
        if (command === 'check_game_mod_updates') return { updates, managed_count: 1 };
        if (command === 'preview_game_mod_updates') return { plan_id: 'stale-plan', changes: [{ project_id: 'map-project', name: 'Forest Map', from_version: '1.0', to_version: '2.0', filename: 'map.jar', enabled: true }] };
        if (command === 'apply_game_mod_updates') return { updated: 1, added: 0 };
        if (command === 'recognize_game_mods') {
          if (target.__failRecognition) throw new Error('synthetic recognition failure');
          if (recognitionBlocked) await new Promise<void>((resolve) => { finishRecognition = resolve; });
          return { recognized: 1, unmatched: 1 };
        }
        if (command === 'set_game_mod_enabled') return localMods;
        if (command === 'remove_game_mod') return localMods;
        if (command === 'import_game_mods') return localMods;
        if (command === 'start_game') return { ...game, launch_state: 'preparing' };
        return null;
      },
    };
  }, { game, mods, updates });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
  await page.getByRole('tab', { name: '更新', exact: true }).click();
}
async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}

test('recognition is explicit, clears stale update state, and requires a manual recheck', async ({ page }, testInfo) => {
  await setup(page);
  expect(await calls(page, 'recognize_game_mods')).toHaveLength(0);
  expect(await calls(page, 'check_game_mod_updates')).toHaveLength(0);
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await page.getByRole('checkbox', { name: '选择更新 Forest Map' }).check();
  await page.getByRole('button', { name: '更新 Forest Map', exact: true }).click();
  await expect(page.getByRole('heading', { name: /本次更新/ })).toBeVisible();
  await page.getByRole('button', { name: /返回更新列表/ }).click();
  await expect(page.getByRole('checkbox', { name: '选择更新 Forest Map' })).toBeChecked();
  await page.getByRole('button', { name: '识别本地 Mod', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('已识别 1 个 Mod，1 个未匹配，需自行确认版本。');
  expect((await calls(page, 'recognize_game_mods')).at(-1)?.args).toEqual({ id: game.id });
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('recognition-result-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('recognition-result-dark-560.png'), fullPage: true, animations: 'disabled' });
  await expect(page.getByRole('list', { name: '可用更新' })).toHaveCount(0);
  await expect(page.getByRole('heading', { name: /本次更新/ })).toHaveCount(0);
  expect(await calls(page, 'check_game_mod_updates')).toHaveLength(1);
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  expect(await calls(page, 'check_game_mod_updates')).toHaveLength(2);
  await page.getByRole('checkbox', { name: '选择更新 Forest Map' }).check();
  await page.getByRole('button', { name: '更新 Forest Map', exact: true }).click();
  await page.getByRole('button', { name: '确认更新', exact: true }).click();
  await expect(page.getByRole('status')).toHaveCount(1);
  await expect(page.getByRole('status')).toHaveText('已更新 1 个 Mod。');
  await expect(page.getByRole('status')).not.toContainText('已识别');
});

test('recognition failure retains candidates and selection for retry', async ({ page }) => {
  await setup(page);
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await page.getByRole('checkbox', { name: '选择更新 Forest Map' }).check();
  await page.evaluate(() => { (window as unknown as Mock).__failRecognition = true; });
  await page.getByRole('button', { name: '识别本地 Mod', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('未能识别本地 Mod');
  await expect(page.getByRole('checkbox', { name: '选择更新 Forest Map' })).toBeChecked();
  await expect(page.getByRole('list', { name: '可用更新' })).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__failRecognition = false; });
  await page.getByRole('button', { name: '重试识别', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('已识别 1 个 Mod');
  expect(await calls(page, 'recognize_game_mods')).toHaveLength(2);
});

test('pending recognition blocks launch, back navigation and local content actions', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => (window as unknown as Mock).__blockNextRecognition());
  await page.getByRole('button', { name: '识别本地 Mod', exact: true }).click();
  await expect(page.getByRole('button', { name: '识别中…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '识别 Mod 中', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '返回游戏库' })).toBeDisabled();
  await expect(page.getByRole('tab', { name: /^已安装/ })).toBeDisabled();
  await expect(page.locator('#installed-content button').filter({ hasText: '导入 Mod' })).toBeDisabled();
  await page.evaluate(() => (window as unknown as Mock).__finishRecognition());
  await expect(page.getByRole('status')).toContainText('已识别 1 个 Mod');
});
