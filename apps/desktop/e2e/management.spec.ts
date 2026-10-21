// SPDX-License-Identifier: MIT
// Synthetic native boundary; these tests never launch, export, or delete real game files.
import { test, expect, type Page } from '@playwright/test';
import type { Game, ProfileLibrary } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type ManagementWindow = Window & {
  __calls: Call[];
  __games: Game[];
  __exportResult: boolean;
  __removeError: boolean;
};
const initialGame: Game = {
  id: 'management-game', name: '测试世界', version: '1.21.1', directory: 'C:\\Synthetic\\world',
  status: 'installed', error: null, progress: null,
};
const profiles: ProfileLibrary = { profiles: [], active_id: null };

async function mockManagementNative(page: Page, game = initialGame) {
  await page.addInitScript(({ initialGame, profiles }) => {
    const target = window as unknown as ManagementWindow & {
      isTauri: boolean;
      __TAURI_INTERNALS__: unknown;
      __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
    };
    target.isTauri = true;
    target.__calls = [];
    target.__games = [structuredClone(initialGame)];
    target.__exportResult = false;
    target.__removeError = false;
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => {
        const id = callbacks.size + 1;
        callbacks.set(id, callback);
        return id;
      },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') return 1;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return structuredClone(target.__games);
        if (command === 'list_player_profiles') return profiles;
        if (command === 'list_game_mods') return [];
        if (command === 'copy_game') {
          const copy: Game = {
            ...initialGame,
            id: 'independent-game-copy',
            name: args.name as string,
            directory: 'C:\\Synthetic\\independent-copy',
            status: 'installed',
            launch_state: 'idle',
          };
          target.__games = [...target.__games, copy];
          return copy;
        }
        if (command === 'export_game') return target.__exportResult;
        if (command === 'remove_game') {
          if (target.__removeError) throw new Error('synthetic disk error');
          target.__games = target.__games.filter((item) => item.id !== args.id);
          return null;
        }
        return null;
      },
    };
  }, { initialGame: game, profiles });
}

async function openActions(page: Page, action: '复制游戏' | '导出整合包' | '移除游戏') {
  await page.getByRole('article', { name: initialGame.name, exact: true }).getByRole('button', { name: '管理内容', exact: true }).click();
  await page.getByRole('button', { name: '更多游戏操作', exact: true }).click();
  await page.getByRole('menuitem', { name: action, exact: true }).click();
}

test('copy makes an independent game without saves by default and export cancellation stays pending', async ({ page }, testInfo) => {
  await mockManagementNative(page);
  await page.addInitScript(() => localStorage.setItem('cobble.appearance', 'light'));
  await page.goto('/');
  await openActions(page, '复制游戏');
  await expect(page.getByRole('heading', { name: '复制游戏', exact: true })).toBeVisible();
  await expect(page.getByLabel('副本名称')).toHaveValue('测试世界 副本');
  await expect(page.getByLabel('包含存档')).not.toBeChecked();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('copy-game-light.png'), fullPage: true, animations: 'disabled' });
  await page.setViewportSize({ width: 560, height: 420 });
  await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; localStorage.setItem('cobble.appearance', 'dark'); window.scrollTo(0, 0); });
  await page.screenshot({ path: testInfo.outputPath('copy-game-dark-560.png'), fullPage: true, animations: 'disabled' });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('button', { name: '创建副本', exact: true }).click();
  await expect(page.getByRole('article', { name: '测试世界 副本' })).toBeVisible();
  const copyCall = await page.evaluate(() => (window as unknown as ManagementWindow).__calls.find((call) => call.command === 'copy_game'));
  expect(copyCall?.args).toEqual({ id: initialGame.id, name: '测试世界 副本', includeSaves: false });
  expect(await page.evaluate(() => (window as unknown as ManagementWindow).__games.map((game) => game.id))).toEqual([initialGame.id, 'independent-game-copy']);
  expect(await page.evaluate(() => (window as unknown as ManagementWindow).__games[0]?.directory)).toBe(initialGame.directory);

  await openActions(page, '导出整合包');
  await expect(page.getByRole('heading', { name: '导出整合包', exact: true })).toBeVisible();
  await expect(page.getByLabel('包含存档')).not.toBeChecked();
  await page.getByRole('button', { name: '选择位置并导出', exact: true }).click();
  await expect(page.getByRole('heading', { name: '导出整合包', exact: true })).toBeVisible();
  await expect(page.getByText('整合包已导出', { exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => (window as unknown as ManagementWindow).__calls.filter((call) => call.command === 'export_game').map((call) => call.args))).toEqual([
    { id: initialGame.id, includeSaves: false },
  ]);
});

test('removal defaults to keeping files', async ({ page }) => {
  await mockManagementNative(page);
  await page.goto('/');
  await openActions(page, '移除游戏');
  await expect(page.getByRole('heading', { name: '移除游戏', exact: true })).toBeVisible();
  await expect(page.getByLabel('仅从游戏库移除')).toBeChecked();
  await page.getByRole('button', { name: '从游戏库移除', exact: true }).click();
  expect(await page.evaluate(() => (window as unknown as ManagementWindow).__calls.find((call) => call.command === 'remove_game')?.args)).toEqual({ id: initialGame.id, deleteFiles: false });
  await expect(page.getByRole('article', { name: '测试世界' })).toHaveCount(0);
});

test('explicit removal choice deletes game files', async ({ page }, testInfo) => {
  await mockManagementNative(page);
  await page.addInitScript(() => localStorage.setItem('cobble.appearance', 'light'));
  await page.goto('/');
  await openActions(page, '移除游戏');
  await page.getByLabel('连同文件删除').check();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('remove-game-delete-choice-light.png'), fullPage: true, animations: 'disabled' });
  await page.setViewportSize({ width: 560, height: 420 });
  await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; localStorage.setItem('cobble.appearance', 'dark'); window.scrollTo(0, 0); });
  await page.screenshot({ path: testInfo.outputPath('remove-game-delete-choice-dark-560.png'), fullPage: true, animations: 'disabled' });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('button', { name: '删除游戏和文件', exact: true }).click();
  expect(await page.evaluate(() => (window as unknown as ManagementWindow).__calls.find((call) => call.command === 'remove_game')?.args)).toEqual({ id: initialGame.id, deleteFiles: true });
  await expect(page.getByRole('article', { name: '测试世界' })).toHaveCount(0);
});

test('failed removal keeps the game-management page and selected choice', async ({ page }) => {
  await mockManagementNative(page);
  await page.goto('/');
  await openActions(page, '移除游戏');
  await page.getByLabel('连同文件删除').check();
  await page.evaluate(() => { (window as unknown as ManagementWindow).__removeError = true; });
  await page.getByRole('button', { name: '删除游戏和文件', exact: true }).click();
  await expect(page.getByRole('heading', { name: '移除游戏', exact: true })).toBeVisible();
  await expect(page.getByLabel('连同文件删除')).toBeChecked();
  await expect(page.getByRole('alert')).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as ManagementWindow).__games.map((game) => game.id))).toEqual([initialGame.id]);
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await page.getByRole('button', { name: '返回游戏库', exact: true }).click();
  await expect(page.getByRole('article', { name: '测试世界', exact: true })).toHaveCount(1);
});

test('running games cannot open management actions', async ({ page }) => {
  await mockManagementNative(page, { ...initialGame, launch_state: 'running' });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容', exact: true }).click();
  await page.getByRole('button', { name: '更多游戏操作', exact: true }).click();
  for (const action of ['复制游戏', '导出整合包', '移除游戏']) {
    await expect(page.getByRole('menuitem', { name: action, exact: true })).toBeDisabled();
  }
});

test('games installing content cannot open management actions', async ({ page }) => {
  await mockManagementNative(page, { ...initialGame, content_installing: true });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容', exact: true }).click();
  await page.getByRole('button', { name: '更多游戏操作', exact: true }).click();
  for (const action of ['复制游戏', '导出整合包', '移除游戏']) {
    await expect(page.getByRole('menuitem', { name: action, exact: true })).toBeDisabled();
  }
});
