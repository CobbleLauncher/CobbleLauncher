// SPDX-License-Identifier: MIT
// Browser UI integration with a synthetic Tauri boundary; this does not install Minecraft.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';

const initialGame: Game = { id: 'synthetic-game', name: '我的世界', version: '1.21.1', directory: 'C:\\Games\\synthetic-game', status: 'installing', error: null, progress: { phase: 'metadata', completed: 0, total: 0, reused: 0 } };
type Call = { command: string; args: Record<string, unknown> };
type TestWindow = Window & { __calls: Call[]; __gameEvent: (game: Game) => void; __javaPick: 'valid' | 'cancel' | 'invalid'; __javaReadFails: boolean; __startMode: 'prepare' | 'race' | 'error' };

async function mockNative(page: Page, persisted: Game[] = [], savedProfile: { name: string; uuid: string } | null = null) {
  await page.addInitScript(({ initial, stored, savedProfile }) => {
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
    target.__startMode = 'prepare';
    let offlineProfile: { name: string; uuid: string } | null = savedProfile;
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
        if (command === 'get_offline_profile') return offlineProfile;
        if (command === 'save_offline_profile') { offlineProfile = { name: args.name as string, uuid: 'synthetic-offline-uuid' }; return offlineProfile; }
        if (command === 'remove_offline_profile') { offlineProfile = null; return null; }
        if (command === 'start_game') {
          if (target.__startMode === 'error') throw new Error('Java 无法启动');
          const game: Game = { ...initial, status: 'installed', launch_state: 'preparing' };
          if (target.__startMode === 'race') target.__gameEvent({ ...game, launch_state: 'running' });
          return game;
        }
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
  }, { initial: initialGame, stored: persisted, savedProfile });
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
  expect(createCall?.args).toEqual({ name: '林间小屋', version: '1.21.1', loaderKind: null, loaderVersion: null, source: { source: 'bmclapi', mirror: '' }, useDefaultDirectory: false });
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
  await expect(page.getByRole('button', { name: '设置离线档案' })).toBeVisible();
});

test('offline profile is explicit, validates names, persists across pages, and can be removed', async ({ page }, testInfo) => {
  await mockNative(page, [{ ...initialGame, status: 'installed' }]);
  await page.goto('/');
  await page.getByRole('button', { name: '设置离线档案' }).click();
  await expect(page.getByRole('heading', { name: '设置', exact: true })).toBeVisible();
  await expect(page.getByLabel('游戏昵称')).toHaveValue('Player');
  await page.getByLabel('游戏昵称').fill('你好');
  await page.getByRole('button', { name: '保存档案', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('昵称须为');
  await page.getByLabel('游戏昵称').fill('Local_Player');
  await page.getByRole('button', { name: '保存档案', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('档案已保存');
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
  const titlebar = await page.locator('.titlebar').boundingBox();
  const heading = await page.getByRole('heading', { name: '设置', exact: true }).boundingBox();
  expect(titlebar?.y).toBeLessThanOrEqual(1);
  expect(heading!.y).toBeGreaterThan(titlebar!.y + titlebar!.height);
  await page.screenshot({ path: testInfo.outputPath('offline-profile-settings-viewport.png') });
  await page.screenshot({ path: testInfo.outputPath('offline-profile-settings.png'), fullPage: true });
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await expect(page.getByRole('button', { name: /Local_Player\s*离线/ })).toBeVisible();
  await expect(page.getByRole('button', { name: '开始游戏', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: '打开设置' }).click();
  await expect(page.getByLabel('游戏昵称')).toHaveValue('Local_Player');
  await page.getByRole('button', { name: '移除档案' }).click();
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await expect(page.getByRole('button', { name: '创建离线档案' })).toBeVisible();
  const call = await page.evaluate(() => (window as unknown as TestWindow).__calls.find((item) => item.command === 'save_offline_profile'));
  expect(call?.args).toEqual({ name: 'Local_Player' });
});

test('launch follows host preparation, running, exit, errors and a newer event winning the invoke race', async ({ page }, testInfo) => {
  await mockNative(page, [{ ...initialGame, status: 'installed' }]);
  await page.goto('/');
  await page.getByRole('button', { name: '创建离线档案' }).click();
  await page.getByRole('button', { name: '保存档案', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('档案已保存');
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await page.getByRole('button', { name: '开始游戏', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('正在准备启动…');
  await expect(page.getByRole('button', { name: '启动中…', exact: true })).toBeDisabled();
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...initialGame, status: 'installed', launch_state: 'running', log_path: 'synthetic.log' } satisfies Game);
  await expect(page.getByRole('button', { name: '运行中', exact: true })).toBeDisabled();
  await page.screenshot({ path: testInfo.outputPath('offline-game-running.png') });
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...initialGame, status: 'installed', launch_state: 'idle' } satisfies Game);
  await expect(page.getByRole('button', { name: '开始游戏', exact: true })).toBeEnabled();
  await page.evaluate(() => { (window as unknown as TestWindow).__startMode = 'error'; });
  await page.getByRole('button', { name: '开始游戏', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('启动失败：Error: Java 无法启动');
  await expect(page.getByRole('button', { name: '开始游戏', exact: true })).toBeEnabled();
  await page.evaluate(() => { (window as unknown as TestWindow).__startMode = 'race'; });
  await page.getByRole('button', { name: '开始游戏', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('游戏运行中');
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(await page.evaluate(() => (window as unknown as TestWindow).__calls.filter((item) => item.command === 'start_game').map((item) => item.args))).toEqual(Array(3).fill({ id: initialGame.id }));
});

test('browser preview cannot save a pretend offline profile', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: '创建离线档案' }).click();
  await page.getByRole('button', { name: '保存档案', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveText('请在桌面应用中保存离线档案');
  await expect(page.getByText('档案已保存', { exact: true })).toHaveCount(0);
});

test('existing local games are shown without requesting versions', async ({ page }) => {
  await mockNative(page, [{ ...initialGame, status: 'installed' }], { name: 'Saved_Player', uuid: 'synthetic-saved-uuid' });
  await page.goto('/');
  await expect(page.getByRole('article', { name: '我的世界' })).toBeVisible();
  await expect(page.getByText('✓ 已安装')).toBeVisible();
  await expect(page.getByRole('button', { name: /Saved_Player\s*离线/ })).toBeVisible();
  await expect(page.getByRole('button', { name: '开始游戏', exact: true })).toBeEnabled();
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
