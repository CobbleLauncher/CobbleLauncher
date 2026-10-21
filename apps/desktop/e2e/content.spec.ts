// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only; these tests do not install or execute game files.
import { test, expect, type Page } from '@playwright/test';
import type { Game, LocalMod } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type MockWindow = Window & { __calls: Call[]; __fabricMode: 'normal' | 'error' | 'race'; __cancelImport: boolean; __emitGame: (game: Game) => void };
const game: Game = { id: 'local-game', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
const mod: LocalMod = { file_name: 'synthetic.jar', name: '林间地图', version: '1.2.3', authors: ['Cobble'], loader: 'fabric', enabled: true, description: '原创测试模组' };
async function mock(page: Page, stored: Game[] = []) {
  await page.addInitScript(({ stored, game, mod }) => {
    const target = window as unknown as MockWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true;
    target.__calls = [];
    target.__fabricMode = 'normal';
    target.__cancelImport = false;
    const callbacks = new Map<number, (event: unknown) => void>();
    let listener = 0;
    let mods: LocalMod[] = [];
    target.__emitGame = (value) => callbacks.get(listener)?.({ event: 'game-install', id: 1, payload: value });
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') { listener = args.handler as number; return 1; }
        if (command === 'list_games') return stored;
        if (command === 'get_offline_profile') return { name: 'Cobble', uuid: 'synthetic' };
        if (command === 'get_download_settings') return { source: 'bmclapi', mirror: '' };
        if (command === 'default_game_directory') return 'synthetic default';
        if (command === 'list_game_versions') return [{ id: '1.21.1', kind: 'release' }, { id: '1.20.6', kind: 'release' }];
        if (command === 'list_fabric_versions') {
          if (target.__fabricMode === 'error') throw new Error('网络不可用');
          if (target.__fabricMode === 'race' && args.version === '1.21.1') {
            await new Promise((resolve) => setTimeout(resolve, 600));
            return [{ version: 'stale', stable: true }];
          }
          return args.version === '1.20.6' ? [{ version: '0.15.11', stable: true }] : [{ version: '0.17.0-beta', stable: false }, { version: '0.16.14', stable: true }];
        }
        if (command === 'create_game') return { ...game, name: args.name, version: args.version, loader_version: args.loaderVersion };
        if (command === 'list_game_mods') return mods;
        if (command === 'import_game_mods') { if (target.__cancelImport) return null; mods = [mod]; return mods; }
        if (command === 'set_game_mod_enabled') { mods = mods.map((item) => ({ ...item, enabled: args.enabled as boolean, file_name: args.enabled ? 'synthetic.jar' : 'synthetic.jar.disabled' })); return mods; }
        if (command === 'remove_game_mod') { mods = []; return mods; }
        if (command === 'start_game') return { ...game, launch_state: 'preparing' };
        if (command === 'plugin:window|is_maximized') return false;
        return null;
      },
    };
  }, { stored, game, mod });
}

test('Fabric chooses stable loader and submits selected version and source', async ({ page }) => {
  await mock(page);
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await expect(page.getByLabel('Minecraft 版本')).toHaveValue('1.21.1');
  await page.getByRole('radio', { name: 'Fabric', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.16.14');
  await page.getByRole('button', { name: '创建并安装' }).click();
  await expect(page.getByRole('article')).toContainText('Fabric 0.16.14');
  const calls = await page.evaluate(() => (window as unknown as MockWindow).__calls);
  expect(calls.find((call) => call.command === 'list_fabric_versions')?.args).toEqual({ version: '1.21.1', source: { source: 'bmclapi', mirror: '' } });
  expect(calls.find((call) => call.command === 'create_game')?.args.loaderVersion).toBe('0.16.14');
});

test('Fabric load failure can retry and return to Vanilla', async ({ page }) => {
  await mock(page);
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await page.evaluate(() => { (window as unknown as MockWindow).__fabricMode = 'error'; });
  await page.getByRole('radio', { name: 'Fabric', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法获取 Fabric 版本');
  await expect(page.getByRole('button', { name: '创建并安装' })).toBeDisabled();
  await page.evaluate(() => { (window as unknown as MockWindow).__fabricMode = 'normal'; });
  await page.getByRole('button', { name: '重试', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.16.14');
  await page.getByRole('radio', { name: '原版', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toHaveCount(0);
  await page.getByRole('button', { name: '创建并安装' }).click();
  expect(await page.evaluate(() => (window as unknown as MockWindow).__calls.find((call) => call.command === 'create_game')?.args.loaderVersion)).toBeNull();
});

test('late Fabric response cannot overwrite a newly selected Minecraft version', async ({ page }) => {
  await mock(page);
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await page.evaluate(() => { (window as unknown as MockWindow).__fabricMode = 'race'; });
  await page.getByRole('radio', { name: 'Fabric', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toBeDisabled();
  await page.getByLabel('Minecraft 版本').selectOption('1.20.6');
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.15.11');
  await expect(page.getByLabel('Fabric 版本').getByRole('option', { name: 'stale' })).toHaveCount(0);
  await page.waitForTimeout(700);
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.15.11');
});

test('manage local Mods imports, searches, toggles, refreshes and confirms deletion', async ({ page }, testInfo) => {
  await mock(page, [game]);
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
  await expect(page.getByRole('heading', { name: game.name })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByText('还没有 Mod，导入本地 .jar 文件开始。')).toBeVisible();
  await page.evaluate(() => { (window as unknown as MockWindow).__cancelImport = true; });
  await page.getByRole('button', { name: '导入 Mod' }).click();
  await expect(page.getByRole('switch')).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as MockWindow).__cancelImport = false; });
  await page.getByRole('button', { name: '导入 Mod' }).click();
  await expect(page.getByRole('heading', { name: mod.name })).toBeVisible();
  await page.getByRole('searchbox').fill('not-found');
  await expect(page.getByText('没有找到匹配的 Mod。')).toBeVisible();
  await page.getByRole('searchbox').fill('Cobble');
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('switch').click();
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  await expect(page.getByText('synthetic.jar.disabled')).toBeVisible();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  await page.getByRole('searchbox').clear();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('local-mods-light.png'), fullPage: true });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: testInfo.outputPath('local-mods-dark.png'), fullPage: true });
  await page.getByRole('button', { name: `删除 ${mod.name}` }).click();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(page.getByRole('switch')).toBeVisible();
  await page.getByRole('button', { name: `删除 ${mod.name}` }).click();
  await page.getByRole('button', { name: '确认删除', exact: true }).click();
  await expect(page.getByRole('switch')).toHaveCount(0);
  const calls = await page.evaluate(() => (window as unknown as MockWindow).__calls);
  expect(calls.find((call) => call.command === 'set_game_mod_enabled')?.args).toEqual({ id: game.id, fileName: 'synthetic.jar', enabled: false });
  expect(calls.find((call) => call.command === 'remove_game_mod')?.args).toEqual({ id: game.id, fileName: 'synthetic.jar.disabled' });
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await expect(page.getByRole('article')).toBeVisible();
});

test('local content mutations lock during start and running; Vanilla hint and installed entry only', async ({ page }) => {
  await mock(page, [{ ...game, loader_version: null }, { ...game, id: 'installing-game', name: '安装中', status: 'installing' }]);
  await page.goto('/');
  await expect(page.getByRole('button', { name: '管理内容' })).toHaveCount(1);
  await page.getByRole('button', { name: '管理内容' }).click();
  await expect(page.getByText('原版不会加载 Mod。需要使用 Fabric 等模组加载器。')).toBeVisible();
  await page.getByRole('button', { name: '导入 Mod' }).click();
  await page.getByRole('button', { name: '开始游戏' }).click();
  await expect(page.getByRole('button', { name: '导入 Mod' })).toBeDisabled();
  await expect(page.getByRole('switch')).toBeDisabled();
  await expect(page.getByRole('button', { name: `删除 ${mod.name}` })).toBeDisabled();
  await page.evaluate((value) => (window as unknown as MockWindow).__emitGame({ ...value, launch_state: 'running' }), game);
  await expect(page.getByRole('button', { name: '运行中', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '导入 Mod' })).toBeDisabled();
  await page.evaluate((value) => (window as unknown as MockWindow).__emitGame({ ...value, launch_state: 'idle' }), game);
  await expect(page.getByRole('button', { name: '导入 Mod' })).toBeEnabled();
});
