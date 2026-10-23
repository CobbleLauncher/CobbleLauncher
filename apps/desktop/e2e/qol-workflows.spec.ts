// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary: exercises App navigation and presentation without touching game files.
import { expect, test, type Page } from '@playwright/test';
import type { Game, LocalWorld } from '../src/games';
import type { TaskSnapshot } from '../src/tasks';

type Call = { command: string; args: Record<string, unknown> };
type FixtureWindow = Window & { __qolCalls: Call[] };
const game: Game = { id: 'qol-game', name: 'Copper Valley', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'idle', launch_error: 'Java 21 is missing', log_path: 'synthetic.log' };
const world: LocalWorld = { folder_name: 'Copper Valley', name: 'Copper Valley', minecraft_version: '1.21.1', game_mode: 0, hardcore: false, last_played: null, metadata_error: false };
const repairTask: TaskSnapshot = { id: 'repair-task-1', game_id: game.id, game_name: game.name, title: 'Repair game files', kind: 'game_repair', state: 'running', progress: { phase: 'assets', completed: 18, total: 60, reused: 2 }, error: null, can_retry: false, created_at: 1, updated_at: 2, has_result: false };

async function mockApp(page: Page, language: string, theme: string, withRepair = false) {
  await page.addInitScript(({ language, theme, initialGame, initialWorld, withRepair, initialRepair }) => {
    const target = window as unknown as FixtureWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__qolCalls = [];
    localStorage.setItem('cobble-language', language);
    localStorage.setItem('cobble.appearance', theme);
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    let nextCallback = 0;
    const callbacks = new Map<number, (event: unknown) => void>();
    const listeners = new Map<string, number>();
    let currentRepair = withRepair ? initialRepair : null;
    let preference = 'undecided';
    let undone = false;
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = ++nextCallback; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__qolCalls.push({ command, args });
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), args.handler as number); return 1; }
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [initialGame];
        if (command === 'list_tasks') return currentRepair ? [currentRepair] : [];
        if (command === 'cancel_task' && currentRepair?.id === args.taskId) {
          currentRepair = { ...currentRepair, state: 'cancelling', updated_at: 3 };
          callbacks.get(listeners.get('task-updated') ?? -1)?.({ event: 'task-updated', id: 1, payload: currentRepair });
          return currentRepair;
        }
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_launch_recovery') return { kind: 'java', current_log: true, can_repair: withRepair };
        if (command === 'open_current_launch_log') return null;
        if (command === 'list_game_mod_history') return [{ operation_id: 'operation-1', created_at: 1792656000000, status: undone ? 'undone' : 'applied', files: [{ name: 'fixture.jar', before: null, after: 'sha256:fixture', enabled: true }] }];
        if (command === 'preview_game_mod_operation_undo') return [];
        if (command === 'undo_game_mod_operation') { undone = true; return null; }
        if (command === 'list_game_worlds') return [initialWorld];
        if (command === 'list_game_world_backups' || command === 'list_world_snapshots') return [];
        if (command === 'get_automatic_world_backup') return { preference, last_backup: null, last_error: null, world_backups: {} };
        if (command === 'set_automatic_world_backup') { preference = String(args.preference); return { preference, last_backup: null, last_error: null, world_backups: {} }; }
        if (command === 'list_plugins') return [{ enabled: true, manifest: { id: 'config-doctor', frontend: {}, permissions: [{ permission: 'launcher:game-keybindings/read' }, { permission: 'launcher:game-keybindings/write' }] } }];
        if (command === 'default_game_directory') return 'C:/Synthetic games';
        if (command === 'list_game_versions') return [{ id: '1.21.1', kind: 'release' }];
        if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
        if (command === 'create_game') return { ...initialGame, id: 'created-game', name: String(args.name), launch_error: null, log_path: null };
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'C:/Synthetic games', custom_game_directory: false };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
        if (command.startsWith('list_')) return [];
        if (command === 'start_game') throw new Error('Synthetic fixture must not launch Minecraft');
        return null;
      },
    };
  }, { language, theme, initialGame: game, initialWorld: world, withRepair, initialRepair: repairTask });
}

async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as FixtureWindow).__qolCalls.filter(call => call.command === name), command);
}

for (const { language, theme, width } of [
  { language: 'en-US', theme: 'light', width: 1100 },
  { language: 'zh-CN', theme: 'dark', width: 560 },
]) {
  const en = language === 'en-US';
  test(`recovery, Mod history, and automatic world backups in ${language} ${theme} ${width}px`, async ({ page }, info) => {
    await mockApp(page, language, theme);
    await page.setViewportSize({ width, height: 850 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/');
    await page.getByRole('button', { name: en ? 'Manage Copper Valley' : '管理 Copper Valley', exact: true }).click();
    const recovery = page.locator('.launch-recovery');
    await expect(recovery).toContainText('Java 21 is missing');
    await expect(recovery.getByRole('button', { name: en ? 'Choose compatible Java' : '选择兼容的 Java' })).toBeVisible();
    await recovery.getByRole('button', { name: en ? 'Open launch log' : '打开本次日志' }).click();
    expect((await calls(page, 'open_current_launch_log'))[0]?.args).toEqual({ id: game.id });
    await page.screenshot({ path: info.outputPath('recovery-and-history.png'), fullPage: true, animations: 'disabled' });

    const history = page.locator('.mod-history');
    await history.getByRole('button', { name: en ? 'Mod history' : 'Mod 历史' }).click();
    await expect(history).toContainText('fixture.jar');
    await history.getByRole('button', { name: en ? 'Review undo' : '预览撤销' }).click();
    expect(await calls(page, 'undo_game_mod_operation')).toHaveLength(0);
    await history.getByRole('button', { name: en ? 'Undo changes' : '撤销更改' }).click();
    expect((await calls(page, 'preview_game_mod_operation_undo'))[0]?.args).toEqual({ id: game.id, operationId: 'operation-1' });
    expect((await calls(page, 'undo_game_mod_operation'))[0]?.args).toEqual({ id: game.id, operationId: 'operation-1' });
    await expect(history).toContainText(en ? 'Undone' : '已撤销');
    await page.screenshot({ path: info.outputPath('mod-history-undone.png'), fullPage: true, animations: 'disabled' });

    await page.getByRole('tab', { name: en ? 'Worlds' : '世界', exact: true }).click();
    const backups = page.locator('.automatic-world-backup');
    await expect(backups).toContainText(en ? 'Back up changed worlds' : '游戏退出后');
    await backups.getByRole('button', { name: en ? 'Enable backups' : '开启自动备份' }).click();
    expect((await calls(page, 'set_automatic_world_backup'))[0]?.args).toEqual({ gameId: game.id, preference: 'enabled' });
    await expect(backups).toContainText(en ? 'Automatic world backups' : '自动备份世界');
    await page.screenshot({ path: info.outputPath('automatic-world-backup.png'), fullPage: true, animations: 'disabled' });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(await calls(page, 'start_game')).toHaveLength(0);
  });

  test(`created game offers keybinding reuse in ${language} ${theme} ${width}px`, async ({ page }, info) => {
    await mockApp(page, language, theme);
    await page.setViewportSize({ width, height: 850 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/');
    await page.getByRole('button', { name: en ? 'Add' : '添加', exact: true }).click();
    await page.getByRole('menuitem', { name: en ? 'Create game' : '创建游戏', exact: true }).click();
    await page.getByLabel(en ? 'Game name' : '游戏名称').fill('New World');
    await page.getByRole('button', { name: en ? 'Create and install' : '创建并安装' }).click();
    const prompt = page.locator('.created-game-preferences');
    await expect(prompt.getByRole('button', { name: en ? 'Use keys from another game' : '使用其他游戏的按键' })).toBeVisible();
    await page.screenshot({ path: info.outputPath('created-game-preferences.png'), fullPage: true, animations: 'disabled' });
    await prompt.getByRole('button', { name: en ? 'Dismiss' : '关闭' }).click();
    await expect(prompt).toHaveCount(0);
    expect((await calls(page, 'create_game'))[0]?.args).toMatchObject({ name: 'New World', version: '1.21.1' });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  });
}

test('running game repair shows live progress and cancels its own task', async ({ page }, info) => {
  await mockApp(page, 'zh-CN', 'dark', true);
  await page.setViewportSize({ width: 560, height: 850 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.getByRole('button', { name: '管理 Copper Valley', exact: true }).click();
  const recovery = page.locator('.launch-recovery');
  await expect(recovery).toHaveAttribute('aria-busy', 'true');
  await expect(page.locator('.details-page .play-button')).toBeDisabled();
  await expect(recovery.getByRole('button', { name: '正在修复游戏文件…' })).toBeDisabled();
  await expect(recovery.locator('.repair-progress')).toContainText('下载游戏资源');
  await expect(recovery.locator('progress')).toHaveAttribute('value', '18');
  await expect(recovery.locator('progress')).toHaveAttribute('max', '60');
  await page.screenshot({ path: info.outputPath('repair-progress-dark-560.png'), fullPage: true, animations: 'disabled' });
  await recovery.locator('.repair-progress').getByRole('button', { name: '取消', exact: true }).click();
  expect((await calls(page, 'cancel_task'))[0]?.args).toEqual({ taskId: repairTask.id });
  await expect(recovery.locator('.repair-progress').getByRole('button', { name: '取消', exact: true })).toBeDisabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
