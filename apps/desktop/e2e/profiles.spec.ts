// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary: these tests never start a game process.
import { test, expect, type Page } from '@playwright/test';
import type { Game, PlayerProfile, ProfileLibrary } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type ProfileWindow = Window & { __calls: Call[]; __failProfileSave: boolean; __gameEvent: (game: Game) => void };
const dylan: PlayerProfile = { id: 'dylan', kind: 'offline', name: 'Dylan', uuid: 'synthetic-dylan', note: null };
const builder: PlayerProfile = { id: 'builder', kind: 'offline', name: 'Builder', uuid: 'synthetic-builder', note: '建筑世界' };
const game: Game = { id: 'test-world', name: '林间小屋', version: '1.21.1', directory: 'C:\\Synthetic\\test-world', status: 'installed', progress: null, error: null };

async function native(page: Page, library: ProfileLibrary = { profiles: [], active_id: null }, initialGame = game) {
  await page.addInitScript(({ initial, game }) => {
    const target = window as unknown as ProfileWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    const library: ProfileLibrary = JSON.parse(sessionStorage.getItem('test-profiles') || JSON.stringify(initial));
    let currentGame: Game = JSON.parse(sessionStorage.getItem('test-game') || JSON.stringify(game));
    let handler = 0;
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__calls = [];
    target.__failProfileSave = false;
    target.isTauri = true;
    target.__gameEvent = (game) => { currentGame = game; callbacks.get(handler)?.({ event: 'game-install', id: 1, payload: game }); };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    const save = () => { sessionStorage.setItem('test-profiles', JSON.stringify(library)); return structuredClone(library); };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') { handler = args.handler as number; return 1; }
        if (command === 'list_games') return [currentGame];
        if (command === 'list_game_mods') return [];
        if (command === 'get_launcher_preferences') return { memory_mb: 2048, game_directory: 'C:\\Cobble\\games', custom_game_directory: false };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_player_profiles') return structuredClone(library);
        if (['add_offline_profile', 'select_player_profile', 'update_profile_note', 'remove_player_profile'].includes(command) && target.__failProfileSave) throw new Error('synthetic disk failure');
        if (command === 'add_offline_profile') {
          const profile: PlayerProfile = { id: `local-${args.name}`, name: args.name as string, kind: 'offline', uuid: `synthetic-${args.name}`, note: args.note as string | null };
          library.profiles.push(profile); library.active_id = profile.id; return save();
        }
        if (command === 'select_player_profile') { library.active_id = args.id as string; return save(); }
        if (command === 'update_profile_note') { library.profiles = library.profiles.map((profile) => profile.id === args.id ? { ...profile, note: args.note as string | null } : profile); return save(); }
        if (command === 'remove_player_profile') { library.profiles = library.profiles.filter((profile) => profile.id !== args.id); if (library.active_id === args.id) library.active_id = null; return save(); }
        if (command === 'set_game_profile') { currentGame = { ...currentGame, profile_id: args.profileId as string | null }; sessionStorage.setItem('test-game', JSON.stringify(currentGame)); return currentGame; }
        if (command === 'start_game') return { ...currentGame, launch_state: 'preparing' };
        return null;
      },
    };
  }, { initial: library, game: initialGame });
}

async function menu(page: Page, label: string) {
  await page.getByRole('button', { name: '切换玩家档案', exact: true }).click();
  await page.getByRole('menuitem', { name: label, exact: true }).click();
}

test('add keeps two identities, exact duplicate reuses, keyboard switch and reload restore selection', async ({ page }, testInfo) => {
  await native(page);
  await page.goto('/');
  await menu(page, '添加档案');
  await expect(page.getByRole('heading', { name: '添加玩家档案', exact: true })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByLabel('游戏昵称').fill('你好');
  await page.getByRole('button', { name: '添加并使用' }).click();
  await expect(page.getByRole('alert')).toContainText('3–16');
  await page.getByLabel('游戏昵称').fill('Dylan');
  await page.getByRole('button', { name: '添加并使用' }).click();
  await expect(page.getByRole('button', { name: '切换玩家档案' })).toContainText('Dylan');
  await menu(page, '添加档案');
  await page.getByLabel('游戏昵称').fill('Builder');
  await page.getByLabel('备注').fill('建筑世界');
  await page.getByRole('button', { name: '添加并使用' }).click();
  await menu(page, '添加档案');
  await page.getByLabel('游戏昵称').fill('Dylan');
  await page.getByRole('button', { name: '使用此档案' }).click();
  const trigger = page.getByRole('button', { name: '切换玩家档案' });
  await trigger.focus(); await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('menu')).toBeVisible();
  await page.keyboard.press('b'); await page.keyboard.press('Enter');
  await expect(trigger).toContainText('Builder');
  await trigger.click(); await page.keyboard.press('Escape');
  await expect(page.getByRole('menu')).toHaveCount(0);
  await page.reload();
  await expect(trigger).toContainText('Builder');
  await menu(page, '管理档案');
  await expect(page.getByRole('list', { name: '玩家档案', exact: true }).getByRole('listitem')).toHaveCount(2);
  await expect(page.getByText('建筑世界', { exact: true })).toBeVisible();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('profiles-light.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.setViewportSize({ width: 560, height: 420 });
  await expect(page.getByRole('button', { name: '管理 Builder' })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('profiles-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '返回', exact: true }).click();
  await page.getByRole('button', { name: '切换玩家档案' }).click();
  await page.screenshot({ path: testInfo.outputPath('profile-switcher-560.png'), animations: 'disabled' });
  await page.keyboard.press('Escape');
  expect(await page.evaluate(() => (window as unknown as ProfileWindow).__calls.some((call) => call.command === 'start_game'))).toBe(false);
});

test('first launch adds identity then returns to the game without starting; cancel is unchanged', async ({ page }) => {
  await native(page); await page.goto('/');
  await page.getByRole('button', { name: '选择玩家档案', exact: true }).click();
  await page.getByLabel('游戏昵称').fill('Dylan');
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(page.getByRole('heading', { name: game.name, exact: true })).toBeVisible();
  await page.getByRole('button', { name: '选择玩家档案', exact: true }).click();
  await page.getByLabel('游戏昵称').fill('Dylan');
  await page.getByRole('button', { name: '添加并使用' }).click();
  await expect(page.getByRole('button', { name: '以 Dylan 开始', exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as ProfileWindow).__calls.some((call) => call.command === 'start_game'))).toBe(false);
  await page.getByRole('button', { name: '以 Dylan 开始', exact: true }).click();
  await expect(page.getByRole('button', { name: '启动中…', exact: true })).toBeDisabled();
});

test('fixed identity is displayed; removing it never falls back and selection repairs binding', async ({ page }) => {
  await native(page, { profiles: [dylan, builder], active_id: builder.id }, { ...game, profile_id: dylan.id });
  await page.goto('/');
  await expect(page.getByRole('article').getByText('使用 Dylan · 离线', { exact: true })).toBeVisible();
  await menu(page, '管理档案');
  await page.getByRole('button', { name: '管理 Dylan', exact: true }).click();
  await page.getByRole('menuitem', { name: '移除档案', exact: true }).click();
  await expect(page.getByText('移除档案 Dylan？游戏和存档会保留。')).toBeVisible();
  await page.getByRole('button', { name: '确认移除' }).click();
  await page.getByRole('button', { name: '返回', exact: true }).click();
  await expect(page.getByRole('article').getByText('请选择玩家档案', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '选择玩家档案', exact: true }).click();
  await expect(page.getByRole('heading', { name: '玩家档案', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '使用 Builder' }).click();
  await expect(page.getByRole('button', { name: '以 Builder 开始' })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as ProfileWindow).__calls.filter((call) => call.command === 'set_game_profile').map((call) => call.args))).toEqual([{ id: game.id, profileId: builder.id }]);
  expect(await page.evaluate(() => (window as unknown as ProfileWindow).__calls.some((call) => call.command === 'start_game'))).toBe(false);
});

test('per-game binding, note failure retaining draft, and removing active selection', async ({ page }) => {
  await native(page, { profiles: [dylan, builder], active_id: dylan.id }); await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
  await page.getByRole('tab', { name: '运行设置', exact: true }).click();
  await page.getByLabel('使用的档案').selectOption(builder.id);
  await expect(page.getByText('实际使用 Builder · 离线', { exact: true })).toBeVisible();
  await page.getByLabel('使用的档案').selectOption('');
  await expect(page.getByText('实际使用 Dylan · 离线', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await menu(page, '管理档案');
  await page.getByRole('button', { name: '管理 Dylan' }).click();
  await page.getByRole('menuitem', { name: '修改备注' }).click();
  await page.getByLabel('备注', { exact: true }).fill('保留输入');
  await page.evaluate(() => { (window as unknown as ProfileWindow).__failProfileSave = true; });
  await page.getByRole('button', { name: '保存备注' }).click();
  await expect(page.getByRole('alert')).toHaveText('未能保存档案，请重试');
  await expect(page.getByLabel('备注', { exact: true })).toHaveValue('保留输入');
  await page.evaluate(() => { (window as unknown as ProfileWindow).__failProfileSave = false; });
  await page.getByRole('button', { name: '保存备注' }).click();
  await expect(page.getByText('保留输入', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '管理 Dylan' }).click();
  await page.getByRole('menuitem', { name: '移除档案', exact: true }).click();
  await page.getByRole('button', { name: '确认移除' }).click();
  await page.getByRole('button', { name: '返回', exact: true }).click();
  await expect(page.getByRole('button', { name: '切换玩家档案' })).toContainText('选择玩家档案');
  await expect(page.getByRole('button', { name: '选择玩家档案', exact: true })).toBeVisible();
});
