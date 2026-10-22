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
  __backupPreview: { preview_id: string; name: string; minecraft_version: string | null; file_count: number; total_bytes: number; suggested_folder: string; metadata_error?: boolean } | null;
  __backupPickerResult: 'backup' | 'cancel';
  __restoreResult: { worlds: LocalWorld[]; rollback_id: string | null };
  __backups: { backup_id: string; folder_name: string; world_name: string; created_at: number }[];
  __blockRestore: () => void;
  __finishRestore: () => void;
  __blockExport: () => void;
  __finishExport: () => void;
  __setGame: (game: Game) => void;
};
const game: Game = { id: 'world-test-game', name: '林间小屋', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'idle' };
const worlds: LocalWorld[] = [
  { folder_name: 'Copper Valley', name: '铜谷', minecraft_version: '1.21.1', game_mode: 0, hardcore: false, last_played: 1_792_595_400_000, metadata_error: false },
  { folder_name: 'Broken Save', name: 'Broken Save', minecraft_version: null, game_mode: null, hardcore: false, last_played: null, metadata_error: true },
];
const backupPreview = { preview_id: 'opaque-world-backup', name: '河谷', minecraft_version: '1.20.1', file_count: 23, total_bytes: 1048576, suggested_folder: 'River Valley' };
const restoredWorlds: LocalWorld[] = [...worlds, { folder_name: 'River Valley', name: '河谷', minecraft_version: '1.20.1', game_mode: 0, hardcore: false, last_played: null, metadata_error: false }];
const replacementWorlds: LocalWorld[] = [{ folder_name: 'Copper Valley', name: '河谷', minecraft_version: '1.20.1', game_mode: 0, hardcore: false, last_played: null, metadata_error: false }, worlds[1]!];
const backupEntry = { backup_id: 'rollback-world-1', folder_name: 'Copper Valley', world_name: '铜谷', created_at: 1_792_595_400_000 };

async function setup(page: Page, initialGame = game, initialWorlds = worlds) {
  await page.addInitScript(({ initialGame, initialWorlds }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__worlds = structuredClone(initialWorlds); target.__failCommand = ''; target.__exportResult = false;
    target.__backupPreview = null; target.__backupPickerResult = 'backup'; target.__restoreResult = { worlds: structuredClone(initialWorlds), rollback_id: null }; target.__backups = [];
    let currentGame = structuredClone(initialGame);
    let blocked = false;
    let finish: () => void = () => {};
    let restoreBlocked = false;
    let finishRestore: () => void = () => {};
    const listeners = new Map<string, number>();
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__blockExport = () => { blocked = true; };
    target.__finishExport = () => { blocked = false; finish(); };
    target.__blockRestore = () => { restoreBlocked = true; };
    target.__finishRestore = () => { restoreBlocked = false; finishRestore(); };
    target.__setGame = (value) => {
      currentGame = value;
      callbacks.get(listeners.get('game-install') ?? -1)?.({ event: 'game-install', id: 1, payload: value });
    };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === target.__failCommand) throw new Error('synthetic world failure');
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), args.handler as number); return 1; }
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [currentGame];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Player', uuid: 'synthetic-player', note: null }], active_id: 'test-player' };
        if (command === 'list_game_mods') return [];
        if (command === 'list_game_worlds') return structuredClone(target.__worlds);
        if (command === 'list_game_world_backups') return structuredClone(target.__backups);
        if (command === 'preview_world_backup') return target.__backupPickerResult === 'cancel' ? null : structuredClone(target.__backupPreview);
        if (command === 'restore_game_world') {
          if (restoreBlocked) await new Promise<void>((resolve) => { finishRestore = resolve; });
          target.__worlds = structuredClone(target.__restoreResult.worlds);
          if (target.__restoreResult.rollback_id) target.__backups = [{ backup_id: target.__restoreResult.rollback_id, folder_name: 'Copper Valley', world_name: '铜谷', created_at: Date.now() }];
          return structuredClone(target.__restoreResult);
        }
        if (command === 'rollback_game_world_restore') { target.__worlds = structuredClone(target.__restoreResult.worlds); return structuredClone(target.__restoreResult); }
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
  expect(await calls(page, 'list_game_world_backups')).toHaveLength(0);
  await openWorlds(page);
  expect(await calls(page, 'list_game_world_backups')).toHaveLength(1);
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
  await expect(page.getByRole('button', { name: '导出 铜谷 的备份', exact: true })).toBeDisabled();
  await expect(page.getByText('导出中…', { exact: true })).toBeVisible();
  await page.evaluate(() => (window as unknown as Mock).__finishExport());
  await expect(page.getByRole('button', { name: '导出 铜谷 的备份', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: '以 Player 开始', exact: true })).toBeEnabled();
  await page.evaluate((running) => (window as unknown as Mock).__setGame(running), { ...game, launch_state: 'running' } satisfies Game);
  await expect(page.getByRole('button', { name: '导入世界备份', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: '打开 铜谷 文件夹', exact: true }).click();
  await expect(page.getByRole('button', { name: '导出 铜谷 的备份', exact: true })).toBeDisabled();
  await expect(page.getByText('关闭游戏并等待当前任务完成后，即可导出或恢复备份。')).toBeVisible();
});

test('cancelled backup selection keeps the world list and search query', async ({ page }) => {
  await setup(page);
  await openWorlds(page);
  await page.getByRole('searchbox', { name: '搜索世界' }).fill('Broken');
  await page.evaluate(() => { (window as unknown as Mock).__backupPickerResult = 'cancel'; });
  await page.getByRole('button', { name: '导入世界备份', exact: true }).click();
  await expect(page.getByRole('searchbox', { name: '搜索世界' })).toHaveValue('Broken');
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('Broken Save');
  await expect(page.getByRole('list', { name: '本地世界' })).not.toContainText('铜谷');
  expect(await calls(page, 'restore_game_world')).toHaveLength(0);
});

test('new-world restore is the default and keeps the confirmed preview editable at 560px', async ({ page }, testInfo) => {
  await setup(page);
  await openWorlds(page);
  await page.evaluate(({ preview, restored }) => {
    const mock = window as unknown as Mock;
    mock.__backupPreview = preview;
    mock.__restoreResult = { worlds: restored, rollback_id: null };
  }, { preview: backupPreview, restored: restoredWorlds });
  await page.getByRole('button', { name: '导入世界备份', exact: true }).click();
  await expect(page.getByRole('heading', { name: '恢复世界', exact: true })).toBeVisible();
  await expect(page.getByText('23 个文件')).toBeVisible();
  await expect(page.getByRole('radio', { name: '恢复为新世界', exact: true })).toBeChecked();
  await expect(page.getByLabel('世界文件夹')).toHaveValue('River Valley');
  await page.getByLabel('世界文件夹').fill('Restored Valley');
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('world-restore-preview-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('world-restore-preview-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.evaluate(() => (window as unknown as Mock).__blockRestore());
  await page.getByRole('button', { name: '恢复世界', exact: true }).click();
  await expect(page.getByRole('button', { name: '恢复中…', exact: true })).toBeDisabled();
  await expect(page.getByLabel('世界文件夹')).toBeDisabled();
  await expect(page.getByRole('tab', { name: '内容', exact: true })).toBeDisabled();
  await page.evaluate(() => (window as unknown as Mock).__finishRestore());
  await expect(page.getByRole('status')).toContainText('世界已恢复');
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('河谷');
  expect((await calls(page, 'restore_game_world')).at(-1)?.args).toEqual({ id: game.id, previewId: backupPreview.preview_id, folderName: 'Restored Valley', replaceExisting: false });
});

test('replacement requires a selected target and explicit confirmation, then offers rollback', async ({ page }, testInfo) => {
  await setup(page);
  await openWorlds(page);
  await page.evaluate(({ preview, replaced, backups }) => {
    const mock = window as unknown as Mock;
    mock.__backupPreview = preview;
    mock.__restoreResult = { worlds: replaced, rollback_id: 'rollback-1' };
    mock.__backups = backups;
  }, { preview: backupPreview, replaced: replacementWorlds, backups: [backupEntry] });
  await page.getByRole('button', { name: '导入世界备份', exact: true }).click();
  await page.getByRole('radio', { name: '替换已有世界', exact: true }).check();
  const target = page.getByLabel('目标世界');
  await target.selectOption('Copper Valley');
  const replaceButton = page.getByRole('button', { name: '替换并保留备份', exact: true });
  await expect(replaceButton).toBeDisabled();
  await page.getByRole('checkbox', { name: /我确认替换/ }).check();
  await expect(replaceButton).toBeEnabled();
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('world-restore-replace-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('world-restore-replace-dark-560.png'), fullPage: true, animations: 'disabled' });
  await replaceButton.click();
  expect((await calls(page, 'restore_game_world')).at(-1)?.args).toEqual({ id: game.id, previewId: backupPreview.preview_id, folderName: 'Copper Valley', replaceExisting: true });
  await expect(page.getByRole('status')).toContainText('世界已恢复');
  await page.getByRole('button', { name: '恢复替换前的世界', exact: true }).click();
  await expect(page.getByRole('region', { name: '恢复备份确认' })).toBeVisible();
  await page.setViewportSize({ width: 560, height: 800 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('world-restore-rollback-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('world-restore-rollback-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.evaluate((original) => { (window as unknown as Mock).__restoreResult = { worlds: original, rollback_id: null }; }, worlds);
  await page.evaluate(() => { (window as unknown as Mock).__backups = []; });
  await page.getByRole('button', { name: '确认恢复备份', exact: true }).click();
  expect((await calls(page, 'rollback_game_world_restore')).at(-1)?.args).toEqual({ id: game.id, rollbackId: 'rollback-1' });
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText('铜谷');
});

test('backup read and restore errors retain the form for retry, and a cancelled replacement picker keeps edits', async ({ page }) => {
  await setup(page);
  await openWorlds(page);
  await page.evaluate(({ preview }) => {
    const mock = window as unknown as Mock;
    mock.__backupPreview = { ...preview, metadata_error: true };
    mock.__failCommand = 'preview_world_backup';
  }, { preview: backupPreview });
  await page.getByRole('button', { name: '导入世界备份', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法读取世界备份');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '选择世界备份 ZIP' }).click();
  await expect(page.getByText('无法读取世界信息，请确认备份来源')).toBeVisible();
  await page.getByLabel('世界文件夹').fill('Kept Folder');
  await page.evaluate(() => { (window as unknown as Mock).__backupPickerResult = 'cancel'; });
  await page.getByRole('button', { name: /更换备份/ }).click();
  await expect(page.getByLabel('世界文件夹')).toHaveValue('Kept Folder');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'restore_game_world'; });
  await page.getByRole('button', { name: '恢复世界', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('恢复世界未完成');
  await expect(page.getByLabel('世界文件夹')).toHaveValue('Kept Folder');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '恢复世界', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('世界已恢复');
});
