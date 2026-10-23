// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only. Verifies App integration, not disk snapshots,
// encryption, OS credentials, native dialogs, or real world restoration.
import { expect, test, type Page } from '@playwright/test';
import type { Game, LocalWorld } from '../src/games';
import type { WorldSnapshot } from '../src/world-snapshots';
type Call = { command: string; args: Record<string, unknown> };
type SnapshotWindow = Window & { __snapshotCalls: Call[]; __finishSnapshotSave: () => void };
const game: Game = { id: 'snapshot-game', name: '版本测试', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'idle' };
const world: LocalWorld = { folder_name: '世界 Copper Valley', name: '铜谷', minecraft_version: '1.21.1', game_mode: 0, hardcore: false, last_played: null, metadata_error: false };
async function mockSnapshots(page: Page) {
  await page.addInitScript(({ initialGame, initialWorld }) => {
    const target = window as unknown as SnapshotWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__snapshotCalls = [];
    localStorage.setItem('cobble-language', 'zh-CN');
    const callbacks = new Map<number, (event: unknown) => void>();
    const listeners = new Map<string, number>();
    const snapshots: WorldSnapshot[] = [];
    let replaced = false;
    target.__finishSnapshotSave = () => {};
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__snapshotCalls.push({ command, args });
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), args.handler as number); return 1; }
        if (command === 'list_tasks') return [];
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [initialGame];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'snapshot-player', kind: 'offline', name: 'Player', uuid: 'synthetic-player', note: null }], active_id: 'snapshot-player' };
        if (command === 'list_game_mods') return [];
        if (command === 'list_game_worlds') return [initialWorld];
        if (command === 'list_game_world_backups') return replaced ? [{ backup_id: 'rollback-before-snapshot', folder_name: initialWorld.folder_name, world_name: initialWorld.name, created_at: 1792656000000 }] : [];
        if (command === 'list_world_snapshots') return structuredClone(snapshots);
        if (command === 'create_world_snapshot') {
          const task = { id: 'snapshot-task-1', game_id: initialGame.id, world_folder: initialWorld.folder_name, game_name: initialGame.name, title: 'Save world version', kind: 'world_snapshot', state: 'running', progress: { phase: 'world-snapshot-write', completed: 1024, total: 8192, reused: 0, detail: null }, error: null, can_retry: false, created_at: 1, updated_at: 1, has_result: false };
          const emitTask = (payload: unknown) => callbacks.get(listeners.get('task-updated') ?? -1)?.({ event: 'task-updated', id: 1, payload });
          emitTask(task);
          await new Promise<void>(resolve => { target.__finishSnapshotSave = () => { emitTask({ ...task, state: 'succeeded', updated_at: 2 }); resolve(); }; });
          const snapshot = { id: 'opaque-snapshot-1', world_id: 'opaque-world-hash', name: String(args.name), created_at: '2026-10-22T08:00:00Z', processed_bytes: 8192, added_bytes: 1024 };
          snapshots.push(snapshot); return structuredClone(snapshot);
        }
        if (command === 'restore_world_snapshot') {
          replaced = true;
          return { worlds: [initialWorld], rollback_id: 'rollback-before-snapshot' };
        }
        if (command === 'export_world_snapshot_recovery' || command === 'import_world_snapshot_recovery') return true;
        if (command === 'start_game') throw new Error('A synthetic integration test must never launch a game');
        return null;
      },
    };
  }, { initialGame: game, initialWorld: world });
}
async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as SnapshotWindow).__snapshotCalls.filter(call => call.command === name), command);
}

test('world versions integrate with game locks, explicit restore, recovery dialogs, and portable backups at narrow width', async ({ page }, testInfo) => {
  await mockSnapshots(page);
  await page.setViewportSize({ width: 560, height: 860 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容', exact: true }).click();
  await page.getByRole('tab', { name: '世界', exact: true }).click();
  await expect(page.getByRole('list', { name: '本地世界' })).toContainText(world.name);
  await page.getByRole('button', { name: '版本历史', exact: true }).click();
  await expect(page.getByRole('heading', { name: '铜谷 · 版本历史', exact: true })).toBeVisible();
  expect((await calls(page, 'list_world_snapshots'))[0]?.args).toEqual({ gameId: game.id, folderName: world.folder_name });
  await page.getByLabel('版本名称', { exact: true }).fill('升级前');
  await page.getByRole('button', { name: '保存版本', exact: true }).click();
  await expect(page.getByRole('button', { name: '保存中…', exact: true })).toBeDisabled();
  await expect(page.locator('.play-button')).toBeDisabled();
  await expect(page.locator('.snapshot-task')).toContainText('保存版本内容');
  await expect(page.locator('.snapshot-task')).toContainText('1 KiB / 8 KiB');
  await expect(page.locator('.snapshot-task').getByRole('button', { name: '取消任务', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: '返回游戏库', exact: true })).toBeDisabled();
  await expect(page.getByRole('tab', { name: '内容', exact: true })).toBeDisabled();
  await expect(page.getByRole('tab', { name: '运行设置', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '← 返回世界列表', exact: true })).toBeDisabled();
  expect((await calls(page, 'create_world_snapshot'))[0]?.args).toEqual({ gameId: game.id, folderName: world.folder_name, name: '升级前' });
  expect(await calls(page, 'start_game')).toHaveLength(0);
  await page.evaluate(() => (window as unknown as SnapshotWindow).__finishSnapshotSave());
  await expect(page.getByText('版本已保存', { exact: true })).toBeVisible();
  await expect(page.locator('.snapshot-list')).toContainText('新增内容 1 KiB');
  await expect(page.locator('.snapshot-list')).toContainText('世界大小 8 KiB');
  await expect(page.locator('.play-button')).toBeEnabled();
  await expect(page.getByRole('button', { name: '返回游戏库', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: '恢复此版本', exact: true }).click();
  await expect(page.locator('.snapshot-confirm')).toContainText('将替换当前世界');
  expect(await calls(page, 'restore_world_snapshot')).toHaveLength(0);
  await page.getByRole('button', { name: '取消', exact: true }).click();
  expect(await calls(page, 'restore_world_snapshot')).toHaveLength(0);
  await page.getByRole('button', { name: '恢复此版本', exact: true }).click();
  await page.getByRole('button', { name: '确认恢复', exact: true }).click();
  await expect(page.locator('.world-snapshots').getByRole('status')).toHaveText('世界已恢复');
  expect((await calls(page, 'restore_world_snapshot'))[0]?.args).toEqual({ gameId: game.id, folderName: world.folder_name, snapshotId: 'opaque-snapshot-1', replaceExisting: true });
  await page.locator('.snapshot-recovery summary').click();
  await expect(page.locator('.snapshot-recovery')).toContainText('请妥善保管密钥');
  await page.getByRole('button', { name: '导出恢复密钥', exact: true }).click();
  await expect(page.getByText('恢复密钥已导出', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '导入恢复密钥', exact: true }).click();
  await expect(page.getByText('恢复密钥已导入', { exact: true })).toBeVisible();
  expect((await calls(page, 'export_world_snapshot_recovery'))[0]?.args).toEqual({ gameId: game.id });
  expect((await calls(page, 'import_world_snapshot_recovery'))[0]?.args).toEqual({ gameId: game.id });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('world-snapshot-history-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '← 返回世界列表', exact: true }).click();
  await expect(page.getByRole('heading', { name: '替换前的备份', exact: true })).toBeVisible();
  await expect(page.locator('.world-backup-list')).toContainText(world.name);
  await expect(page.getByRole('button', { name: '恢复替换前的世界', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: `导出 ${world.name} 的备份`, exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: '导入世界备份', exact: true })).toBeEnabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('world-snapshot-prior-backup-560.png'), fullPage: true, animations: 'disabled' });
});
