// SPDX-License-Identifier: MIT
// Synthetic native boundary; no Modrinth requests or game files are used.
import { test, expect, type Page } from '@playwright/test';
import type { Game, LocalMod } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & {
  __calls: Call[];
  __fail: string;
  __failFile: string;
  __finishApply: () => void;
  __blockNextApply: () => void;
  __emitRunning: () => void;
};
const game: Game = { id: 'forest', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
const mods: LocalMod[] = [
  { file_name: 'map.jar', name: 'Forest Map', version: '1.0', authors: ['Forest'], loader: 'fabric', enabled: true, description: 'Map', pack_owned: true },
  { file_name: 'library.jar.disabled', name: 'Map Library', version: '2.0', authors: ['Forest'], loader: 'fabric', enabled: false, description: 'Dependency', pack_owned: false },
];
const updates = [
  { project_id: 'map-project', name: 'Forest Map', current_version: '1.0', version_number: '2.0', version_id: 'map-v2', filename: 'map.jar', enabled: true, pack_owned: true },
  { project_id: 'library-project', name: 'Map Library', current_version: '2.0', version_number: '3.0', version_id: 'lib-v3', filename: 'library.jar.disabled', enabled: false, pack_owned: false },
];

async function setup(page: Page) {
  await page.addInitScript(({ game, mods, updates }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__fail = ''; target.__failFile = '';
    let listener = 0;
    let localMods = [...mods];
    let applyBlocked = false;
    const callbacks = new Map<number, (event: unknown) => void>();
    const emit = (payload: Game) => callbacks.get(listener)?.({ event: 'game-install', id: 1, payload });
    target.__emitRunning = () => emit({ ...game, launch_state: 'running' });
    let finishApply: () => void = () => {};
    target.__finishApply = () => finishApply();
    target.__blockNextApply = () => { applyBlocked = true; };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (target.__fail === command) throw new Error('synthetic unavailable');
        if (command === 'plugin:event|listen') { listener = args.handler as number; return 1; }
        if (command === 'list_games') return [game];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Player', uuid: 'synthetic', note: null }], active_id: 'test-player' };
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_game_mods') return localMods;
        if (command === 'check_game_mod_updates') return { updates, managed_count: 2 };
        if (command === 'preview_game_mod_updates') {
          const ids = args.projectIds as string[];
          return { plan_id: 'opaque-plan-7f3', changes: [
            ...(ids.includes('map-project') ? [{ project_id: 'map-project', name: 'Forest Map', from_version: '1.0', to_version: '2.0', filename: 'map.jar', enabled: true }] : []),
            { project_id: 'library-project', name: 'Map Library', from_version: ids.includes('library-project') ? '2.0' : null, to_version: '3.0', filename: 'library.jar.disabled', enabled: false },
          ] };
        }
        if (command === 'apply_game_mod_updates') {
          if (applyBlocked) await new Promise<void>((resolve) => { finishApply = resolve; });
          return { updated: 1, added: 1 };
        }
        if (command === 'set_game_mod_enabled') {
          if (target.__failFile === args.fileName) throw new Error('synthetic file failure');
          const enabled = args.enabled as boolean;
          localMods = localMods.map((mod) => {
            if (mod.file_name !== args.fileName) return mod;
            const fileName = String(args.fileName);
            return { ...mod, enabled, file_name: enabled ? fileName.replace(/\.disabled$/, '') : fileName.endsWith('.disabled') ? fileName : `${fileName}.disabled` };
          });
          return localMods;
        }
        if (command === 'remove_game_mod') {
          if (target.__failFile === args.fileName) throw new Error('synthetic file failure');
          localMods = localMods.filter((mod) => mod.file_name !== args.fileName); return localMods;
        }
        if (command === 'import_game_mods') return localMods;
        if (command === 'start_game') return { ...game, launch_state: 'preparing' };
        return null;
      },
    };
  }, { game, mods, updates });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
}
async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}

test('updates are checked only on demand, preview dependencies, and apply an opaque plan', async ({ page }, info) => {
  await setup(page);
  expect(await calls(page, 'check_game_mod_updates')).toHaveLength(0);
  await page.getByRole('tab', { name: '更新', exact: true }).click();
  expect(await calls(page, 'check_game_mod_updates')).toHaveLength(0);
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await expect(page.getByRole('list', { name: '可用更新' })).toBeVisible();
  expect((await calls(page, 'check_game_mod_updates'))[0].args).toEqual({ id: game.id });
  const packRow = page.getByRole('listitem').filter({ hasText: 'Forest Map' });
  await expect(packRow.getByText('整合包自带', { exact: true })).toBeVisible();
  await expect(packRow.getByRole('checkbox', { name: '选择更新 Forest Map' })).toBeEnabled();
  await page.getByRole('checkbox', { name: '选择更新 Forest Map' }).check();
  await page.getByRole('button', { name: '更新所选' }).click();
  await expect(page.getByRole('heading', { name: /本次更新/ })).toBeVisible();
  await expect(page.getByRole('list', { name: '更新内容' })).toContainText('新增依赖');
  await expect(page.getByRole('list', { name: '更新内容' })).toContainText('保持停用');
  expect((await calls(page, 'preview_game_mod_updates'))[0].args).toEqual({ id: game.id, projectIds: ['map-project'] });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.screenshot({ path: info.outputPath('mod-update-preview-light.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: info.outputPath('mod-update-preview-dark.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '确认更新', exact: true }).click();
  expect((await calls(page, 'apply_game_mod_updates')).at(-1)?.args).toEqual({ id: game.id, planId: 'opaque-plan-7f3' });
  await expect(page.getByRole('status')).toContainText('已更新 1 个 Mod');
  expect(await calls(page, 'check_game_mod_updates')).toHaveLength(1);
});

test('check and preview failures retry with selected projects preserved', async ({ page }) => {
  await setup(page);
  await page.getByRole('tab', { name: '更新', exact: true }).click();
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'check_game_mod_updates'; });
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('暂时无法检查更新');
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await page.getByRole('checkbox', { name: '选择更新 Forest Map' }).check();
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'preview_game_mod_updates'; });
  await page.getByRole('button', { name: '更新 Forest Map', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法准备更新');
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '重试准备' }).click();
  await expect(page.getByRole('heading', { name: /本次更新/ })).toBeVisible();
  expect((await calls(page, 'preview_game_mod_updates')).map((call) => call.args)).toEqual([
    { id: game.id, projectIds: ['map-project'] }, { id: game.id, projectIds: ['map-project'] },
  ]);
});

test('failed apply retries the same opaque plan; updates lock local actions and launch while busy', async ({ page }) => {
  await setup(page);
  await page.getByRole('tab', { name: '更新', exact: true }).click();
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await page.getByRole('button', { name: '更新 Forest Map', exact: true }).click();
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'apply_game_mod_updates'; });
  await page.getByRole('button', { name: '确认更新', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('更新未完成');
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '重试更新', exact: true }).click();
  expect((await calls(page, 'apply_game_mod_updates')).map((call) => call.args)).toEqual([
    { id: game.id, planId: 'opaque-plan-7f3' }, { id: game.id, planId: 'opaque-plan-7f3' },
  ]);
});

test('a pending update apply disables starting and local content mutations', async ({ page }) => {
  await setup(page);
  await page.getByRole('tab', { name: '更新', exact: true }).click();
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await page.getByRole('button', { name: '更新 Forest Map', exact: true }).click();
  await page.evaluate(() => (window as unknown as Mock).__blockNextApply());
  await page.getByRole('button', { name: '确认更新', exact: true }).click();
  await expect(page.getByRole('button', { name: '更新中…' })).toBeDisabled();
  await expect(page.getByRole('button', { name: '更新 Mod 中', exact: true })).toBeDisabled();
  await expect(page.getByRole('tab', { name: '已安装' })).toBeDisabled();
  await expect(page.locator('#installed-content button').filter({ hasText: '导入 Mod' })).toBeDisabled();
  expect(await page.locator('#installed-content [role="switch"]').evaluateAll((elements) => elements.every((element) => (element as HTMLButtonElement).disabled))).toBe(true);
  await page.evaluate(() => (window as unknown as Mock).__finishApply());
  await expect(page.getByRole('status')).toContainText('已更新 1 个 Mod');
});

test('bulk local actions use existing per-file commands and do not auto-check updates', async ({ page }) => {
  await setup(page);
  await expect(page.getByRole('list', { name: '本地 Mod' })).toBeVisible();
  expect(await calls(page, 'check_game_mod_updates')).toHaveLength(0);
  await page.getByRole('checkbox', { name: '选择 Forest Map' }).check();
  await page.getByRole('button', { name: '停用所选' }).click();
  await expect(page.getByRole('switch', { name: '启用 Forest Map' })).toHaveAttribute('aria-checked', 'false');
  expect((await calls(page, 'set_game_mod_enabled')).at(-1)?.args).toEqual({ id: game.id, fileName: 'map.jar', enabled: false });
  await page.getByRole('checkbox', { name: '选择 Forest Map' }).check();
  await page.getByRole('button', { name: '移除所选' }).click();
  await expect(page.getByRole('button', { name: '确认移除所选' })).toBeVisible();
  await page.getByRole('button', { name: '确认移除所选' }).click();
  expect((await calls(page, 'remove_game_mod')).at(-1)?.args).toEqual({ id: game.id, fileName: 'map.jar.disabled' });
});

test('bulk enable stops at the first file failure and retains failed selection for retry', async ({ page }) => {
  await setup(page);
  await expect(page.getByRole('list', { name: '本地 Mod' })).toBeVisible();
  await page.getByRole('checkbox', { name: '全选当前列表' }).check();
  await page.evaluate(() => { (window as unknown as Mock).__failFile = 'library.jar.disabled'; });
  await page.getByRole('button', { name: '停用所选' }).click();
  await expect(page.getByRole('alert')).toContainText('未能处理 Map Library');
  await expect(page.getByRole('switch', { name: '启用 Forest Map' })).toHaveAttribute('aria-checked', 'false');
  await expect(page.getByRole('checkbox', { name: '选择 Forest Map' })).not.toBeChecked();
  await expect(page.getByRole('checkbox', { name: '选择 Map Library' })).toBeChecked();
  expect((await calls(page, 'set_game_mod_enabled')).map((call) => call.args)).toEqual([
    { id: game.id, fileName: 'map.jar', enabled: false }, { id: game.id, fileName: 'library.jar.disabled', enabled: false },
  ]);
  expect(await calls(page, 'list_game_mods')).toHaveLength(2);
  await page.evaluate(() => { (window as unknown as Mock).__failFile = ''; });
  await page.getByRole('button', { name: '停用所选' }).click();
  expect((await calls(page, 'set_game_mod_enabled')).at(-1)?.args).toEqual({ id: game.id, fileName: 'library.jar.disabled', enabled: false });
  expect(await calls(page, 'list_game_mods')).toHaveLength(3);
  await expect(page.getByRole('switch', { name: '启用 Map Library' })).toHaveAttribute('aria-checked', 'false');
});
