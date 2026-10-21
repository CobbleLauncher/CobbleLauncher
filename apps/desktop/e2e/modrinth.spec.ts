// SPDX-License-Identifier: MIT
// Synthetic IPC boundary; no external requests or game execution.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';
type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & { __calls: Call[]; __fail: string; __running: () => void; __finish: () => void };
const game: Game = { id: 'forest', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
async function setup(page: Page, vanilla = false) {
  await page.addInitScript(({ game, vanilla }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__fail = '';
    let installed = false;
    let listener = 0;
    const callbacks = new Map<number, (event: unknown) => void>();
    const emit = (payload: Game) => callbacks.get(listener)?.({ event: 'game-install', id: 1, payload });
    target.__running = () => emit({ ...game, launch_state: 'running' });
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (target.__fail === command) throw new Error(command === 'preview_game_mod_install' ? 'Fabric API 与已安装的 Map API 版本冲突' : 'synthetic unavailable');
        if (command === 'plugin:event|listen') { listener = args.handler as number; return 1; }
        if (command === 'list_games') return [{ ...game, loader_version: vanilla ? null : game.loader_version }];
        if (command === 'get_offline_profile') return { name: 'Player', uuid: 'synthetic' };
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_game_mods') return installed ? [{ file_name: 'map.jar', name: 'Forest Map', version: '2.0', authors: ['Forest'], loader: 'fabric', enabled: true, description: 'Map' }] : [];
        if (command === 'search_game_mods') {
          if (args.query === 'slow') await new Promise((resolve) => setTimeout(resolve, 400));
          return { hits: [{ project_id: args.offset ? 'second' : String(args.query || 'map'), slug: 'map', title: args.offset ? 'Second Mod' : String(args.query || 'Forest Map'), description: '<img src=x onerror=alert(1)> A useful map', author: 'Forest', downloads: 120000 }], offset: args.offset, total_hits: 2 };
        }
        if (command === 'preview_game_mod_install') {
          if (args.projectId === 'slow') await new Promise((resolve) => setTimeout(resolve, 400));
          return { plan_id: `plan-${String(args.projectId)}`, project_id: args.projectId, version_id: 'v1', mods: [{ project_id: args.projectId, version_id: 'v1', name: String(args.projectId), version_number: '2.0', required_by: null }, { project_id: 'api', version_id: 'api1', name: 'Fabric API', version_number: '0.100', required_by: String(args.projectId) }] };
        }
        if (command === 'install_game_mod_plan') {
          emit({ ...game, content_installing: true });
          await new Promise<void>((resolve) => { target.__finish = resolve; });
          installed = true; emit({ ...game, content_installing: false });
          return { installed: 2, reused: 0 };
        }
        return null;
      },
    };
  }, { game, vanilla });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
}
async function calls(page: Page, command: string) { return page.evaluate((command) => (window as unknown as Mock).__calls.filter((call) => call.command === command), command); }

test('discovery is on demand, resolves dependencies and installs with a host plan', async ({ page }, info) => {
  await setup(page);
  await expect.poll(async () => (await calls(page, 'list_game_mods')).length).toBe(1);
  expect(await calls(page, 'search_game_mods')).toHaveLength(0);
  await page.getByRole('tab', { name: '发现 Mod' }).click();
  await expect(page.getByText('Minecraft 1.21.1', { exact: true })).toBeVisible();
  expect(await calls(page, 'search_game_mods')).toHaveLength(0);
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('map');
  expect(await calls(page, 'search_game_mods')).toHaveLength(0);
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).press('Enter');
  await expect(page.getByRole('button', { name: '查看 map' })).toBeVisible();
  expect((await calls(page, 'search_game_mods'))[0].args).toEqual({ id: game.id, query: 'map', offset: 0 });
  await expect(page.locator('.discover-list img')).toHaveCount(0);
  await page.getByRole('button', { name: '查看 map' }).click();
  await expect(page.getByText('map 的必需依赖')).toBeVisible();
  await page.screenshot({ path: info.outputPath('modrinth-plan-light.png'), fullPage: true });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: info.outputPath('modrinth-plan-dark.png'), fullPage: true });
  await page.getByRole('button', { name: '安装 2 个 Mod' }).click();
  await expect(page.getByRole('button', { name: '正在安装…' })).toBeDisabled();
  await expect(page.getByRole('button', { name: '安装 Mod 中' })).toBeDisabled();
  await expect(page.getByRole('tab', { name: '已安装' })).toBeDisabled();
  expect((await calls(page, 'install_game_mod_plan'))[0].args).toEqual({ id: game.id, planId: 'plan-map' });
  await page.evaluate(() => (window as unknown as Mock).__finish());
  await expect(page.getByRole('tab', { name: '已安装' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('heading', { name: 'Forest Map' })).toBeVisible();
  expect(await calls(page, 'list_game_mods')).toHaveLength(2);
});

test('search retries, pages, and ignores a late response', async ({ page }) => {
  await setup(page);
  await page.getByRole('tab', { name: '发现 Mod' }).click();
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'search_game_mods'; });
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('暂时无法获取');
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '重试', exact: true }).click();
  await page.getByRole('button', { name: '加载更多' }).click();
  await expect(page.getByRole('button', { name: '查看 Second Mod' })).toBeVisible();
  expect((await calls(page, 'search_game_mods')).at(-1)?.args.offset).toBe(1);
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('slow');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('fast');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('button', { name: '查看 fast' })).toBeVisible();
  await page.waitForTimeout(450);
  await expect(page.getByRole('button', { name: '查看 slow' })).toHaveCount(0);
});

test('preview retries, rejects stale preview, and running prevents installation', async ({ page }) => {
  await setup(page);
  await page.getByRole('tab', { name: '发现 Mod' }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('slow');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByRole('button', { name: '查看 slow' }).click();
  await page.getByRole('button', { name: '← 返回搜索结果' }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('fast');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'preview_game_mod_install'; });
  await page.getByRole('button', { name: '查看 fast' }).click();
  await expect(page.getByRole('alert')).toContainText('Fabric API 与已安装的 Map API 版本冲突');
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '重试', exact: true }).click();
  await page.waitForTimeout(450);
  await expect(page.getByText('fast 的必需依赖')).toBeVisible();
  await expect(page.getByText('slow 的必需依赖')).toHaveCount(0);
  await page.evaluate(() => (window as unknown as Mock).__running());
  await expect(page.getByRole('button', { name: '安装 2 个 Mod' })).toBeDisabled();
});

test('Vanilla provides a clear empty discovery state without requests', async ({ page }) => {
  await setup(page, true);
  await page.getByRole('tab', { name: '发现 Mod' }).click();
  await expect(page.getByText('发现 Mod 适用于 Fabric 游戏。创建 Fabric 游戏后即可浏览和安装。')).toBeVisible();
  expect(await calls(page, 'search_game_mods')).toHaveLength(0);
});


test('failed installation shows its cause and can retry the same plan', async ({ page }) => {
  await setup(page);
  await page.getByRole('tab', { name: '发现 Mod' }).click();
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByRole('button', { name: '查看 Forest Map' }).click();
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'install_game_mod_plan'; });
  await page.getByRole('button', { name: '安装 2 个 Mod' }).click();
  await expect(page.getByRole('alert')).toContainText('安装未完成：Error: synthetic unavailable');
  await expect(page.getByRole('button', { name: '开始游戏' })).toBeEnabled();
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '重试安装' }).click();
  await expect(page.getByRole('button', { name: '正在安装…' })).toBeDisabled();
  const installs = await calls(page, 'install_game_mod_plan');
  expect(installs).toHaveLength(2);
  expect(installs[0].args).toEqual(installs[1].args);
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await expect(page.getByRole('button', { name: '安装 Mod 中' })).toBeDisabled();
  await page.evaluate(() => (window as unknown as Mock).__finish());
  await expect(page.getByRole('button', { name: '开始游戏' })).toBeEnabled();
});
