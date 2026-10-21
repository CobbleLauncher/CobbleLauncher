// SPDX-License-Identifier: MIT
// Browser UI integration with a synthetic Tauri boundary; this does not install Minecraft.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';

const initialGame: Game = { id: 'synthetic-game', name: '我的世界', version: '1.21.1', directory: 'C:\\Games\\synthetic-game', status: 'installing', error: null, progress: { phase: 'metadata', completed: 0, total: 0, reused: 0 } };
type Call = { command: string; args: Record<string, unknown> };
type TestWindow = Window & { __calls: Call[]; __gameEvent: (game: Game) => void };

async function mockNative(page: Page, persisted: Game[] = []) {
  await page.addInitScript(({ initial, stored }) => {
    const target = window as unknown as TestWindow & {
      isTauri: boolean;
      __TAURI_INTERNALS__: unknown;
      __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
    };
    const callbacks = new Map<number, (event: unknown) => void>();
    let handler = 0;
    target.isTauri = true;
    target.__calls = [];
    target.__gameEvent = (game) => callbacks.get(handler)?.({ event: 'game-install', id: 1, payload: game });
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') { handler = args.handler as number; return 1; }
        if (command === 'list_games') return stored;
        if (command === 'get_download_settings') return { source: 'bmclapi', mirror: '' };
        if (command === 'default_game_directory') return 'C:\\Cobble\\games';
        if (command === 'list_game_versions') return [{ id: '1.21.1', kind: 'release' }, { id: '1.20.6', kind: 'release' }];
        if (command === 'pick_game_directory') return 'D:\\My Games';
        if (command === 'create_game') return { ...initial, name: args.name, version: args.version };
        if (command === 'retry_game') return initial;
        if (command === 'plugin:window|is_maximized') return false;
        return null;
      },
    };
  }, { initial: initialGame, stored: persisted });
}

test('browser creation explains desktop requirement without inventing versions or progress', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.getByRole('alert')).toHaveText('请在桌面应用中安装游戏');
  await expect(page.getByLabel('Minecraft 版本')).toBeDisabled();
  await page.getByRole('button', { name: '创建并安装' }).click();
  await expect(page.getByRole('progressbar')).toHaveCount(0);
});

test('create, real event presentation, failure and retry through mocked native boundary', async ({ page }, testInfo) => {
  await mockNative(page);
  await page.goto('/');
  await expect(page.getByRole('heading', { name: '还没有游戏' })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as TestWindow).__calls.some((call) => call.command === 'list_game_versions'))).toBe(false);
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await expect(page.getByLabel('Minecraft 版本')).toHaveValue('1.21.1');
  await expect(page.getByRole('radio', { name: 'BMCLAPI', exact: true })).toHaveAttribute('aria-checked', 'true');
  await page.getByLabel('游戏名称').fill('林间小屋');
  await page.getByRole('button', { name: '更改', exact: false }).click();
  await expect(page.getByRole('button', { name: 'D:\\My Games 更改' })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('create-light.png') });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: testInfo.outputPath('create-dark.png') });
  await page.emulateMedia({ colorScheme: 'light' });
  await page.setViewportSize({ width: 560, height: 420 });
  await page.getByRole('button', { name: '创建并安装' }).scrollIntoViewIfNeeded();
  await expect(page.getByRole('button', { name: '创建并安装' })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.setViewportSize({ width: 680, height: 560 });
  await page.getByRole('button', { name: '创建并安装' }).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.getByRole('article', { name: '林间小屋' })).toBeVisible();
  const createCall = await page.evaluate(() => (window as unknown as TestWindow).__calls.find((call) => call.command === 'create_game'));
  expect(createCall?.args).toEqual({ name: '林间小屋', version: '1.21.1', source: { source: 'bmclapi', mirror: '' }, useDefaultDirectory: false });
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...initialGame, name: '林间小屋', progress: { phase: 'assets', completed: 24, total: 80, reused: 2 } } satisfies Game);
  await expect(page.getByRole('status')).toHaveText('下载游戏资源24 / 80');
  await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '24');
  await page.screenshot({ path: testInfo.outputPath('install-progress-light.png') });
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('radio', { name: '深色', exact: true }).click();
  await page.keyboard.press('Escape');
  await page.screenshot({ path: testInfo.outputPath('install-progress-dark.png') });
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...initialGame, name: '林间小屋', status: 'failed', error: '网络连接已中断' } satisfies Game);
  await expect(page.getByRole('alert')).toHaveText('网络连接已中断');
  await page.getByRole('button', { name: '重试安装' }).click();
  await expect(page.getByRole('progressbar')).toBeVisible();
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...initialGame, status: 'installed', progress: { phase: 'complete', completed: 80, total: 80, reused: 2 } } satisfies Game);
  await expect(page.getByText('✓ 已安装')).toBeVisible();
  await expect(page.getByRole('button', { name: '开始游戏' })).toHaveCount(0);
});

test('existing local games are shown without requesting versions', async ({ page }) => {
  await mockNative(page, [{ ...initialGame, status: 'installed' }]);
  await page.goto('/');
  await expect(page.getByRole('article', { name: '我的世界' })).toBeVisible();
  await expect(page.getByText('✓ 已安装')).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as TestWindow).__calls.some((call) => call.command === 'list_game_versions'))).toBe(false);
});
