// SPDX-License-Identifier: MIT
// Synthetic native boundary only; these tests never read or export real worlds.
import { test, expect, type Page } from '@playwright/test';
import type { Game, LocalWorld } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & {
  __calls: Call[];
  __worlds: LocalWorld[];
  __failCommand: string;
  __exportResult: boolean;
  __blockExport: () => void;
  __finishExport: () => void;
  __setGame: (game: Game) => void;
};
const game: Game = { id: 'world-test-game', name: '林间小屋', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'idle' };
const worlds: LocalWorld[] = [
  { folder_name: 'Copper Valley', name: '铜谷', minecraft_version: '1.21.1', game_mode: 0, hardcore: false, last_played: 1_792_595_400_000, metadata_error: false },
  { folder_name: 'Broken Save', name: 'Broken Save', minecraft_version: null, game_mode: null, hardcore: false, last_played: null, metadata_error: true },
];

async function setup(page: Page, initialGame = game, initialWorlds = worlds) {
  await page.addInitScript(({ initialGame, initialWorlds }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__worlds = structuredClone(initialWorlds); target.__failCommand = ''; target.__exportResult = false;
    let currentGame = structuredClone(initialGame);
    let blocked = false;
    let finish: () => void = () => {};
    let listener = 0;
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__blockExport = () => { blocked = true; };
    target.__finishExport = () => { blocked = false; finish(); };
    target.__setGame = (value) => {
      currentGame = value;
      callbacks.get(listener)?.({ event: 'game-install', id: 1, payload: value });
    };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === target.__failCommand) throw new Error('synthetic world failure');
        if (command === 'plugin:event|listen') { listener = args.handler as number; return 1; }
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [currentGame];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Player', uuid: 'synthetic-player', note: null }], active_id: 'test-player' };
        if (command === 'list_game_mods') return [];
        if (command === 'list_game_worlds') return structuredClone(target.__worlds);
        if (command === 'open_game_world_directory') return null;
        if (command === 'export_game_world') {
          if (blocked) await new Promise<void>((resolve) => { finish = resolve; });
          return target.__exportResult;
        }
        if (command === 'start_game') return { ...currentGame, launch_state: 'preparing' };
        return null;
      },
    };
  }, { initialGame, initialWorlds });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
}
async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}
async function openWorlds(page: Page) {
  await page.getByRole('tab', { name: '世界', exact: true }).click();
  await expect(page.getByRole('heading', { name: /^本地世界/ })).toBeVisible();
}

test('empty games and searches show useful world-list guidance', async ({ page }) => {
  await setup(page, game, []);
  await openWorlds(page);
  await expect(page.getByText('还没有本地世界。在游戏中创建世界后，会显示在这里。')).toBeVisible();
  await page.getByRole('searchbox', { name: '搜索世界' }).fill('missing');
  await expect(page.getByText('没有找到匹配的世界。')).toBeVisible();
});

test('worlds load lazily, search matches display and folder names, and unreadable metadata keeps folder access', async ({ page }) => {
  await setup(page);
  expect(await calls(page, 'list_game_worlds')).toHaveLength(0);
  await openWorlds(page);
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('铜谷');
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('生存');
  await expect(page.getByText('无法读取世界信息')).toBeVisible();
  expect((await calls(page, 'list_game_worlds'))[0].args).toEqual({ id: game.id });
  await page.getByRole('searchbox', { name: '搜索世界' }).fill('Copper Valley');
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('铜谷');
  await expect(page.getByRole('list', { name: '本地世界' })).not.toContainText('Broken Save');
  await page.getByRole('searchbox', { name: '搜索世界' }).fill('Broken');
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('Broken Save');
  await page.getByRole('tab', { name: '内容', exact: true }).click();
  await page.getByRole('tab', { name: '世界', exact: true }).click();
  await expect(page.getByRole('searchbox', { name: '搜索世界' })).toHaveValue('Broken');
  expect(await calls(page, 'list_game_worlds')).toHaveLength(1);
  await page.getByRole('button', { name: '打开 Broken Save 文件夹', exact: true }).click();
  expect((await calls(page, 'open_game_world_directory')).at(-1)?.args).toEqual({ id: game.id, folderName: 'Broken Save' });
});

test('backup is explicit, cancellation shows no success, and successful export is visible at 560px', async ({ page }, testInfo) => {
  await setup(page);
  await openWorlds(page);
  await page.getByRole('button', { name: '导出 铜谷 的备份', exact: true }).click();
  expect((await calls(page, 'export_game_world'))[0].args).toEqual({ id: game.id, folderName: 'Copper Valley' });
  await expect(page.getByText('备份已导出')).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as Mock).__exportResult = true; });
  await page.getByRole('button', { name: '导出 铜谷 的备份', exact: true }).click();
  await expect(page.getByText('备份已导出')).toBeVisible();
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('worlds-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('worlds-dark-560.png'), fullPage: true, animations: 'disabled' });
});

test('read, open and export failures remain retryable without losing the world row', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'list_game_worlds'; });
  await openWorlds(page);
  await expect(page.getByRole('alert')).toContainText('无法读取世界');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '重试读取', exact: true }).click();
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('铜谷');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'open_game_world_directory'; });
  await page.getByRole('button', { name: '打开 铜谷 文件夹', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法打开世界文件夹');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'export_game_world'; });
  await page.getByRole('button', { name: '导出 铜谷 的备份', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法导出世界备份');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '导出 铜谷 的备份', exact: true }).click();
  expect(await calls(page, 'export_game_world')).toHaveLength(2);
});

test('export locks navigation while pending and running games can browse and open worlds but cannot export', async ({ page }) => {
  await setup(page);
  await openWorlds(page);
  await expect(page.getByRole('button', { name: '以 Player 开始', exact: true })).toBeEnabled();
  await page.evaluate(() => (window as unknown as Mock).__blockExport());
  await page.getByRole('button', { name: '导出 铜谷 的备份', exact: true }).click();
  await expect(page.getByRole('button', { name: '导出 铜谷 的备份', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '返回游戏库' })).toBeDisabled();
  await expect(page.getByRole('tab', { name: '内容' })).toBeDisabled();
  await expect(page.getByRole('button', { name: '导出备份中…' })).toBeDisabled();
  await page.evaluate(() => (window as unknown as Mock).__finishExport());
  await expect(page.getByRole('button', { name: '导出 铜谷 的备份', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: '以 Player 开始', exact: true })).toBeEnabled();
  await page.evaluate((running) => (window as unknown as Mock).__setGame(running), { ...game, launch_state: 'running' } satisfies Game);
  await page.getByRole('button', { name: '打开 铜谷 文件夹', exact: true }).click();
  await expect(page.getByRole('button', { name: '导出 铜谷 的备份', exact: true })).toBeDisabled();
  await expect(page.getByText('关闭游戏并等待当前任务完成后，即可导出备份。')).toBeVisible();
});
