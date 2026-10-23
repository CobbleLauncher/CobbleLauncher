// SPDX-License-Identifier: MIT
// Synthetic native boundary only; no Microsoft account or browser is contacted.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type Profile = { id: string; kind: 'offline' | 'microsoft'; name: string; uuid: string; note: string | null };
type Library = { profiles: Profile[]; active_id: string | null };
type Flow = { flow_id: string; user_code: string; verification_uri: string; expires_in: number; interval: number };
type Mock = Window & {
  __calls: Call[];
  __failCommand: string;
  __loginState: 'pending' | 'complete';
  __completeLibrary: Library;
  __blockNextPoll: () => void;
  __finishPoll: () => void;
  __commitLogin: () => void;
};
const game: Game = { id: 'microsoft-auth-game', name: '认证测试游戏', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null };
const firstMicrosoft: Profile = { id: 'ms-alex', kind: 'microsoft', name: 'Alex', uuid: 'synthetic-ms-uuid-alex', note: null };
const secondMicrosoft: Profile = { id: 'ms-casey', kind: 'microsoft', name: 'Casey', uuid: 'synthetic-ms-uuid-casey', note: null };
const offline: Profile = { id: 'offline-builder', kind: 'offline', name: 'Builder', uuid: 'synthetic-offline-uuid', note: null };
const flow: Flow = { flow_id: 'synthetic-flow-1', user_code: 'ABCD-EFGH', verification_uri: 'https://example.invalid/device', expires_in: 120, interval: 1 };
const completeLibrary: Library = { profiles: [firstMicrosoft], active_id: firstMicrosoft.id };

async function setup(page: Page, initial: Library = { profiles: [], active_id: null }) {
  await page.addInitScript(({ game, initial, flow, completeLibrary }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    let library: Library = JSON.parse(sessionStorage.getItem('test-microsoft-profiles') || JSON.stringify(initial));
    let currentGame = structuredClone(game);
    let pollBlocked = false;
    let finishPoll: () => void = () => {};
    let cancelled = false;
    const saveLibrary = () => sessionStorage.setItem('test-microsoft-profiles', JSON.stringify(library));
    target.isTauri = true; target.__calls = JSON.parse(sessionStorage.getItem('test-microsoft-calls') || '[]'); target.__failCommand = ''; target.__loginState = 'pending'; target.__completeLibrary = structuredClone(completeLibrary);
    target.__blockNextPoll = () => { pollBlocked = true; };
    target.__finishPoll = () => { pollBlocked = false; finishPoll(); };
    target.__commitLogin = () => { library = structuredClone(target.__completeLibrary); saveLibrary(); };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args }); sessionStorage.setItem('test-microsoft-calls', JSON.stringify(target.__calls));
        if (target.__failCommand === command) throw new Error('synthetic Microsoft sign-in failure');
        if (command === 'plugin:event|listen') return 1;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [currentGame];
        if (command === 'list_game_mods') return [];
        if (command === 'get_launcher_preferences') return { memory_mb: 2048, game_directory: 'C:\\Cobble\\games', custom_game_directory: false };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_player_profiles') return structuredClone(library);
        if (command === 'begin_microsoft_login') { cancelled = false; return flow; }
        if (command === 'open_microsoft_login') return null;
        if (command === 'cancel_microsoft_login') { cancelled = true; return null; }
        if (command === 'poll_microsoft_login') {
          if (pollBlocked) await new Promise<void>((resolve) => { finishPoll = resolve; });
          if (target.__loginState === 'complete') {
            if (!cancelled) { library = structuredClone(target.__completeLibrary); saveLibrary(); }
            return { state: 'complete', library: structuredClone(target.__completeLibrary) };
          }
          return { state: 'pending' };
        }
        if (command === 'select_player_profile') { library.active_id = args.id as string; saveLibrary(); return structuredClone(library); }
        if (command === 'set_game_profile') { currentGame = { ...currentGame, profile_id: args.profileId as string | null }; return structuredClone(currentGame); }
        if (command === 'start_game') return { ...currentGame, launch_state: 'preparing' };
        return null;
      },
    };
  }, { game, initial, flow, completeLibrary });
  await page.goto('/');
}
async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}
async function addProfile(page: Page) {
  await page.getByRole('button', { name: '切换玩家档案', exact: true }).click();
  await page.getByRole('menuitem', { name: '添加档案', exact: true }).click();
  await expect(page.getByRole('heading', { name: '添加玩家档案', exact: true })).toBeVisible();
}
async function beginMicrosoft(page: Page) {
  await page.getByRole('button', { name: 'Microsoft 登录', exact: true }).click();
  await expect(page.getByLabel('复制登录代码')).toHaveText(flow.user_code);
}

test('Microsoft sign-in begins only after choice, opens the device page, and uses the returned library on completion', async ({ page }, testInfo) => {
  await setup(page);
  expect(await calls(page, 'begin_microsoft_login')).toHaveLength(0);
  expect(await calls(page, 'poll_microsoft_login')).toHaveLength(0);
  expect(await calls(page, 'open_microsoft_login')).toHaveLength(0);
  await addProfile(page);
  await expect(page.getByRole('button', { name: '离线档案', exact: true })).toBeVisible();
  await beginMicrosoft(page);
  expect((await calls(page, 'begin_microsoft_login'))[0].args).toEqual({});
  await expect.poll(async () => (await calls(page, 'open_microsoft_login')).length).toBe(1);
  expect((await calls(page, 'open_microsoft_login'))[0].args).toEqual({ flowId: flow.flow_id });
  expect((await calls(page, 'plugin:clipboard-manager|write_text'))[0].args).toEqual({ text: flow.user_code });
  await expect(page.getByRole('status').filter({ hasText: '已复制' })).toBeVisible();
  await page.getByLabel('复制登录代码').click();
  expect(await calls(page, 'plugin:clipboard-manager|write_text')).toHaveLength(2);
  await expect.poll(async () => (await calls(page, 'poll_microsoft_login')).length).toBeGreaterThan(0);
  expect((await calls(page, 'poll_microsoft_login'))[0].args).toEqual({ flowId: flow.flow_id });
  await expect(page.getByText('等待浏览器授权…')).toBeVisible();
  await expect(page.locator('input[type="password"], input[name*="token" i]')).toHaveCount(0);
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('microsoft-login-waiting-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('microsoft-login-waiting-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '重新打开浏览器', exact: true }).click();
  expect((await calls(page, 'open_microsoft_login')).at(-1)?.args).toEqual({ flowId: flow.flow_id });
  await page.evaluate((library) => {
    (window as unknown as Mock).__completeLibrary = library;
    (window as unknown as Mock).__loginState = 'complete';
  }, completeLibrary);
  await expect(page.getByRole('button', { name: '切换玩家档案', exact: true })).toContainText('Alex');
  await expect(page.getByRole('button', { name: '切换玩家档案', exact: true })).toContainText('正版');
  expect(await calls(page, 'start_game')).toHaveLength(0);
  await page.reload();
  await expect(page.getByRole('button', { name: '切换玩家档案', exact: true })).toContainText('Alex');
  await expect(page.getByRole('button', { name: '切换玩家档案', exact: true })).toContainText('正版');
  expect(await calls(page, 'begin_microsoft_login')).toHaveLength(1);
});

test('back during polling reloads a native-committed account and ignores the late poll response', async ({ page }) => {
  await setup(page);
  await addProfile(page);
  await beginMicrosoft(page);
  await page.evaluate(() => (window as unknown as Mock).__blockNextPoll());
  await expect.poll(async () => (await calls(page, 'poll_microsoft_login')).length).toBeGreaterThan(0);
  await page.evaluate((library) => {
    (window as unknown as Mock).__completeLibrary = library;
    (window as unknown as Mock).__loginState = 'complete';
    (window as unknown as Mock).__commitLogin();
  }, completeLibrary);
  await page.getByRole('button', { name: '返回', exact: true }).click();
  await expect.poll(async () => (await calls(page, 'cancel_microsoft_login')).length).toBe(1);
  expect((await calls(page, 'cancel_microsoft_login'))[0].args).toEqual({ flowId: flow.flow_id });
  await expect(page.getByRole('button', { name: '切换玩家档案', exact: true })).toContainText('Alex');
  await expect(page.getByRole('button', { name: '切换玩家档案', exact: true })).toContainText('正版');
  await page.evaluate(() => (window as unknown as Mock).__finishPoll());
  await expect(page.getByRole('button', { name: '切换玩家档案', exact: true })).toContainText('Alex');
  expect(await calls(page, 'poll_microsoft_login')).toHaveLength(1);
  expect(await calls(page, 'start_game')).toHaveLength(0);
});

test('a failed begin can retry without losing the offline option', async ({ page }) => {
  await setup(page);
  await addProfile(page);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'begin_microsoft_login'; });
  await page.getByRole('button', { name: 'Microsoft 登录', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByRole('button', { name: '离线档案', exact: true })).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '重试登录', exact: true }).click();
  await expect(page.getByLabel('复制登录代码')).toHaveText(flow.user_code);
  expect(await calls(page, 'begin_microsoft_login')).toHaveLength(2);
});

test('saved Microsoft accounts switch and bind to a game with the正版 label', async ({ page }, testInfo) => {
  await setup(page, { profiles: [firstMicrosoft, secondMicrosoft, offline], active_id: offline.id });
  const switcher = page.getByRole('button', { name: '切换玩家档案', exact: true });
  await switcher.click();
  await page.getByRole('menuitem', { name: new RegExp(`^${secondMicrosoft.name}`) }).click();
  await expect(switcher).toContainText(secondMicrosoft.name);
  await expect(switcher).toContainText('正版');
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('microsoft-saved-profile-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('microsoft-saved-profile-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '管理内容', exact: true }).click();
  await page.getByRole('tab', { name: '运行设置', exact: true }).click();
  const profileSelect = page.getByLabel('使用的档案');
  await expect(profileSelect.locator('option', { hasText: '正版' })).toHaveCount(2);
  await profileSelect.selectOption(firstMicrosoft.id);
  await expect(page.getByText('实际使用 Alex · 正版', { exact: true })).toBeVisible();
  expect((await calls(page, 'select_player_profile')).at(-1)?.args).toEqual({ id: secondMicrosoft.id });
  expect((await calls(page, 'set_game_profile')).at(-1)?.args).toEqual({ id: game.id, profileId: firstMicrosoft.id });
  expect(await calls(page, 'start_game')).toHaveLength(0);
});
