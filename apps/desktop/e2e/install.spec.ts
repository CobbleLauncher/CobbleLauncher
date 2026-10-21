// SPDX-License-Identifier: MIT
// Browser UI integration with a synthetic Tauri boundary; this does not install Minecraft.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';

const initialGame: Game = { id: 'synthetic-game', name: '我的世界', version: '1.21.1', directory: 'C:\\Games\\synthetic-game', status: 'installing', error: null, progress: { phase: 'metadata', completed: 0, total: 0, reused: 0 } };
type Call = { command: string; args: Record<string, unknown> };
type TestWindow = Window & { __calls: Call[]; __gameEvent: (game: Game) => void; __javaPick: 'valid' | 'cancel' | 'invalid'; __javaReadFails: boolean };

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
    target.__javaPick = 'valid';
    target.__javaReadFails = false;
    let javaSelection: unknown = { mode: 'automatic', executable: null, runtime: null };
    target.__gameEvent = (game) => callbacks.get(handler)?.({ event: 'game-install', id: 1, payload: game });
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') { handler = args.handler as number; return 1; }
        if (command === 'list_games') return stored;
        if (command === 'get_java_settings') {
          if (target.__javaReadFails) throw new Error('已保存 Java 不存在');
          return javaSelection;
        }
        if (command === 'use_automatic_java') {
          javaSelection = { mode: 'automatic', executable: null, runtime: null };
          return javaSelection;
        }
        if (command === 'pick_java_executable') {
          if (target.__javaPick === 'cancel') return null;
          if (target.__javaPick === 'invalid') throw new Error('无法执行 java -version');
          javaSelection = { mode: 'custom', executable: 'C:\\Java\\bin\\java.exe', runtime: { version: '21.0.7', major: 21, arch: 'amd64', vendor: 'Microsoft', executable: 'C:\\Java\\bin\\java.exe' } };
          return javaSelection;
        }
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
  await expect(page.getByRole('heading', { name: '创建游戏', exact: true })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
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
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('create-light.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: testInfo.outputPath('create-dark.png'), fullPage: true, animations: 'disabled' });
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
  await page.getByRole('button', { name: '返回游戏库' }).click();
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

test('Java selection probes through native picker, survives navigation, and supports automatic choice', async ({ page }, testInfo) => {
  await mockNative(page);
  await page.goto('/');
  expect(await page.evaluate(() => (window as unknown as TestWindow).__calls.some((call) => call.command === 'get_java_settings'))).toBe(false);
  await page.getByRole('button', { name: '打开设置' }).click();
  await expect(page.getByText('按游戏版本选择', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '选择 Java', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Java 21', exact: true })).toBeVisible();
  await expect(page.getByText('Microsoft · 21.0.7 · amd64', { exact: true })).toBeVisible();
  await expect(page.locator('.java-path')).toHaveText('C:\\Java\\bin\\java.exe');
  await page.getByRole('button', { name: '保存并返回' }).scrollIntoViewIfNeeded();
  await expect(page.getByRole('button', { name: '关闭窗口', exact: true })).toBeInViewport();
  await expect(page.getByRole('button', { name: '最小化', exact: true })).toBeInViewport();
  for (const theme of ['浅色', '深色']) {
    await page.getByRole('radio', { name: theme, exact: true }).click();
    await page.screenshot({ path: testInfo.outputPath(theme === '浅色' ? 'settings-java-light.png' : 'settings-java-dark.png'), fullPage: true, animations: 'disabled' });
  }
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await page.getByRole('button', { name: '打开设置' }).click();
  await expect(page.getByRole('heading', { name: 'Java 21', exact: true })).toBeVisible();
  await page.evaluate(() => { (window as unknown as TestWindow).__javaPick = 'cancel'; });
  await page.getByRole('button', { name: '选择 Java', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Java 21', exact: true })).toBeVisible();
  await page.evaluate(() => { (window as unknown as TestWindow).__javaPick = 'invalid'; });
  await page.getByRole('button', { name: '选择 Java', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('请选择有效的 Java 可执行文件');
  await expect(page.getByRole('heading', { name: 'Java 21', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '自动选择', exact: true }).click();
  await expect(page.getByText('按游戏版本选择', { exact: true })).toBeVisible();
  await expect(page.locator('.java-path')).toHaveCount(0);
});

test('unavailable saved Java can be recovered with automatic selection', async ({ page }) => {
  await mockNative(page);
  await page.goto('/');
  await page.evaluate(() => { (window as unknown as TestWindow).__javaReadFails = true; });
  await page.getByRole('button', { name: '打开设置' }).click();
  await expect(page.getByRole('alert')).toContainText('重新选择或恢复自动选择');
  await expect(page.getByRole('button', { name: '选择 Java', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: '自动选择', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
});
