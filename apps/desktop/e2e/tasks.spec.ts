// SPDX-License-Identifier: MIT
// Synthetic native task boundary; no downloads or game processes are started.
import { test, expect, type Page } from '@playwright/test';
import type { Game, PackPreview } from '../src/games';
import type { TaskSnapshot } from '../src/tasks';

type Call = { command: string; args: Record<string, unknown> };
type TaskWindow = Window & {
  __calls: Call[];
  __emitTask: (task: TaskSnapshot) => void;
  __failCommand: string;
  __holdRetry: boolean;
  __finishRetry: () => void;
};
const game: Game = { id: 'task-game', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'C:\\Synthetic\\task-game', status: 'installed', error: null, progress: null };
const pack: PackPreview = { preview_id: 'task-pack-preview', name: '异步下载的整合包', version_id: '1.2.0', summary: '任务中心提供的预览', minecraft_version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', required_files: 2, optional_files: [], overrides: 0 };
const now = Date.now();
const running: TaskSnapshot = { id: 'download-running', game_id: game.id, game_name: game.name, title: '下载游戏资源', kind: 'game_install', state: 'running', progress: { phase: 'assets', completed: 18, total: 60, reused: 2 }, error: null, can_retry: false, created_at: now - 3000, updated_at: now - 1000, has_result: false };
const installer: TaskSnapshot = { id: 'forge-installer', game_id: game.id, game_name: game.name, title: '安装 Forge', kind: 'game_install', state: 'running', progress: { phase: 'forge-installer', completed: 3, total: 90, reused: 0 }, error: null, can_retry: false, created_at: now - 2000, updated_at: now, has_result: false };
const failed: TaskSnapshot = { id: 'failed-task', game_id: game.id, game_name: game.name, title: '更新 Mod', kind: 'mod_update', state: 'failed', progress: null, error: '网络中断', can_retry: true, created_at: now - 5000, updated_at: now - 4000, has_result: false };
const interrupted: TaskSnapshot = { ...failed, id: 'interrupted-task', title: '安装资源包', kind: 'resource_pack', state: 'interrupted', error: '应用已关闭' };
const downloadPack: TaskSnapshot = { id: 'download-pack', game_id: null, game_name: '', title: '下载整合包：异步下载的整合包', kind: 'modpack_download', state: 'succeeded', progress: null, error: null, can_retry: false, created_at: now - 10000, updated_at: now - 8000, has_result: true };

async function setup(page: Page, initial: TaskSnapshot[] = [running, installer, failed, downloadPack]) {
  await page.addInitScript(({ initial, game, pack }) => {
    const target = window as unknown as TaskWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    const callbacks = new Map<number, (event: unknown) => void>();
    const listeners = new Map<string, number>();
    const currentTasks = [...initial];
    let retryProducedMod = false;
    let finishRetry: () => void = () => {};
    target.isTauri = true;
    target.__calls = [];
    target.__failCommand = '';
    target.__holdRetry = false;
    target.__finishRetry = () => finishRetry();
    target.__emitTask = (task) => {
      if (task.id === 'failed-task' && task.state === 'succeeded') retryProducedMod = true;
      const index = currentTasks.findIndex((item) => item.id === task.id);
      if (index < 0) currentTasks.unshift(task); else currentTasks[index] = task;
      const callback = callbacks.get(listeners.get('task-updated') ?? -1);
      callback?.({ event: 'task-updated', id: 1, payload: task });
    };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), args.handler as number); return 1; }
        if (target.__failCommand === command) throw new Error('synthetic task operation failure');
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [game];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'player', kind: 'offline', name: 'Player', uuid: 'synthetic', note: null }], active_id: 'player' };
        if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
        if (command === 'default_game_directory') return 'C:\\Cobble\\games';
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'C:\\Cobble\\games', custom_game_directory: false };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_tasks') return [...currentTasks];
        if (command === 'cancel_task') return { ...currentTasks.find((task) => task.id === args.taskId)!, state: 'cancelling', updated_at: Date.now() };
        if (command === 'retry_task') {
          if (target.__holdRetry) await new Promise<void>((resolve) => { finishRetry = resolve; });
          const task = currentTasks.find((item) => item.id === args.taskId)!;
          return { ...task, state: 'running', error: null, updated_at: Date.now() };
        }
        if (command === 'get_task_pack_preview') return pack;
        if (command === 'list_game_mods') return retryProducedMod ? [{ file_name: 'forest-map.jar', name: 'Forest Map', version: '1.0', authors: ['Forest'], loader: 'fabric', enabled: true, description: 'Added by retry' }] : [];
        if (command === 'list_game_packs') return [];
        if (command === 'search_game_mods') return { hits: [{ project_id: 'map', slug: 'map', title: 'map', description: 'Synthetic map', author: 'Forest', downloads: 1 }], offset: args.offset, total_hits: 1 };
        if (command === 'list_game_mod_versions') return [{ version_id: 'map-v1', name: 'map', version_number: '1.0', version_type: 'release', date_published: '2026-10-20' }];
        if (command === 'preview_game_mod_install') return { plan_id: 'task-preserved-plan', project_id: 'map', version_id: 'map-v1', mods: [{ project_id: 'map', version_id: 'map-v1', name: 'map', version_number: '1.0', required_by: null }] };
        return null;
      },
    };
  }, { initial, game, pack });
}

async function openTasks(page: Page) {
  await page.getByRole('button', { name: '打开任务' }).click();
  await expect(page.getByRole('heading', { name: '任务', exact: true })).toBeVisible();
}

test('completed task locates its game without starting a game', async ({ page }, info) => {
  await setup(page, [{ ...running, id: 'completed-game', state: 'succeeded', progress: null }]);
  await page.setViewportSize({ width: 1100, height: 760 });
  await page.goto('/');
  await openTasks(page);
  await page.screenshot({ path: info.outputPath('tasks-completed-1100.png'), animations: 'disabled' });
  await page.getByRole('button', { name: '查看游戏', exact: true }).click();
  await expect(page.getByRole('heading', { name: game.name, exact: true })).toBeVisible();
  await expect(page.locator('.tasks-workspace')).not.toBeVisible();
  expect(await page.evaluate(() => (window as unknown as TaskWindow).__calls.some(call => ['start_game', 'launch_game'].includes(call.command)))).toBe(false);
});

test('loads task snapshots, counts only active work, and distinguishes known totals from installer phases', async ({ page }, info) => {
  await setup(page);
  await page.goto('/');
  const trigger = page.getByRole('button', { name: '打开任务' });
  await expect(trigger).toContainText('2');
  await expect.poll(async () => (await page.evaluate(() => (window as unknown as TaskWindow).__calls.filter((call) => call.command === 'list_tasks'))).length).toBe(1);
  await openTasks(page);
  await expect(page.getByRole('list', { name: '任务列表' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '下载游戏资源' })).toBeVisible();
  await expect(page.getByRole('progressbar', { name: '下载游戏资源' })).toHaveAttribute('aria-valuenow', '18');
  const forgeRow = page.getByRole('listitem').filter({ has: page.getByRole('heading', { name: '安装 Forge' }) });
  await expect(forgeRow.getByRole('progressbar')).toHaveCount(0);
  await expect(forgeRow.getByText('3 / 90')).toHaveCount(0);
  await page.setViewportSize({ width: 560, height: 800 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: info.outputPath('tasks-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: info.outputPath('tasks-dark-560.png'), fullPage: true, animations: 'disabled' });
});

test('cancellation remains cancelling until a terminal task-updated snapshot arrives', async ({ page }) => {
  await setup(page, [running]);
  await page.goto('/');
  await openTasks(page);
  await page.getByRole('button', { name: '取消任务' }).click();
  await expect(page.getByText('正在取消', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '打开任务' })).toContainText('1');
  const args = await page.evaluate(() => (window as unknown as TaskWindow).__calls.find((call) => call.command === 'cancel_task')?.args);
  expect(args).toEqual({ taskId: running.id });
  await page.evaluate((task) => (window as unknown as TaskWindow).__emitTask(task), { ...running, state: 'cancelled', updated_at: Date.now() });
  await expect(page.getByText('已取消', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '打开任务' })).not.toContainText('1');
});

test('failed and interrupted tasks retry with their own IDs; errors remain retryable', async ({ page }) => {
  await setup(page, [failed, interrupted]);
  await page.goto('/');
  await openTasks(page);
  const failedRow = page.getByRole('listitem').filter({ has: page.getByRole('heading', { name: '更新 Mod' }) });
  await page.evaluate(() => { (window as unknown as TaskWindow).__failCommand = 'retry_task'; });
  await failedRow.getByRole('button', { name: '重试任务' }).click();
  await expect(page.getByRole('alert')).toContainText('synthetic task operation failure');
  await expect(failedRow.getByRole('button', { name: '重试任务' })).toBeEnabled();
  await page.evaluate(() => { (window as unknown as TaskWindow).__failCommand = ''; });
  await failedRow.getByRole('button', { name: '重试任务' }).click();
  await expect(failedRow.getByText('进行中', { exact: true })).toBeVisible();
  const interruptedRow = page.getByRole('listitem').filter({ has: page.getByRole('heading', { name: '安装资源包' }) });
  await interruptedRow.getByRole('button', { name: '重试任务' }).click();
  await expect(interruptedRow.getByText('进行中', { exact: true })).toBeVisible();
  const args = await page.evaluate(() => (window as unknown as TaskWindow).__calls.filter((call) => call.command === 'retry_task').map((call) => call.args));
  expect(args).toEqual([{ taskId: failed.id }, { taskId: failed.id }, { taskId: interrupted.id }]);
});

test('continuing a completed pack task opens its native preview without creating a game', async ({ page }) => {
  await setup(page, [downloadPack]);
  await page.goto('/');
  await openTasks(page);
  await page.getByRole('button', { name: '继续安装' }).click();
  await expect(page.getByRole('heading', { name: '异步下载的整合包', exact: true })).toBeVisible();
  const calls = await page.evaluate(() => (window as unknown as TaskWindow).__calls);
  expect(calls.find((call) => call.command === 'get_task_pack_preview')?.args).toEqual({ taskId: downloadPack.id });
  expect(calls.some((call) => call.command === 'create_mrpack_game')).toBe(false);
});

test('task overlay preserves an open Mod plan and returns without another Mod request', async ({ page }) => {
  await setup(page, [running]);
  await page.goto('/');
  await page.getByRole('article', { name: game.name }).getByRole('button', { name: '管理内容' }).click();
  await page.getByRole('tab', { name: '发现 Mod' }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('map');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByRole('button', { name: '查看 map' }).click();
  await expect(page.getByRole('heading', { name: /^本次安装/ })).toBeVisible();
  const before = await page.evaluate(() => (window as unknown as TaskWindow).__calls.filter((call) => ['search_game_mods', 'list_game_mod_versions', 'preview_game_mod_install'].includes(call.command)).length);
  await openTasks(page);
  await page.getByRole('button', { name: '返回工作区' }).click();
  await expect(page.getByRole('tabpanel', { name: '发现 Mod' }).getByRole('heading', { name: 'map', level: 2 })).toBeVisible();
  await expect(page.getByRole('heading', { name: '本次安装' })).toBeVisible();
  const after = await page.evaluate(() => (window as unknown as TaskWindow).__calls.filter((call) => ['search_game_mods', 'list_game_mod_versions', 'preview_game_mod_install'].includes(call.command)).length);
  expect(after).toBe(before);
});

test('a completed retry refreshes local content when returning to its still-open game details', async ({ page }) => {
  await setup(page, [failed]);
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
  await expect(page.getByRole('tab', { name: '已安装' })).toBeVisible();
  await openTasks(page);
  await page.getByRole('button', { name: '重试任务' }).click();
  await expect(page.getByRole('listitem').getByText('进行中', { exact: true })).toBeVisible();
  await page.evaluate((task) => (window as unknown as TaskWindow).__emitTask(task), { ...failed, state: 'succeeded', error: null, updated_at: Date.now() + 1 });
  await page.getByRole('button', { name: '返回工作区' }).click();
  await expect(page.getByRole('heading', { name: 'Forest Map' })).toBeVisible();
  const reads = await page.evaluate(() => (window as unknown as TaskWindow).__calls.filter((call) => call.command === 'list_game_mods'));
  expect(reads).toHaveLength(2);
});

test('download detail stays collapsed until opened and shows actual file source and bytes', async ({ page }, info) => {
  await setup(page, [{ ...running, progress: { ...running.progress!, detail: { bytesCompleted: 1048576, bytesTotal: 4194304, bytesReceived: 2097152, bytesPerSecond: 524288, activeDownloads: 2, items: [{ source: 'official', item: 'client.jar', bytesReceived: 1048576, bytesTotal: 3145728 }, { source: 'bmclapi', item: 'fabric-loader.jar', bytesReceived: 1048576, bytesTotal: 1048576 }] } } }]);
  await page.goto('/');
  await openTasks(page);
  const details = page.locator('.task-transfer-details');
  await expect(details.locator('summary')).toContainText('512 KB/s');
  await expect(page.getByRole('list', { name: '下载中的文件' })).not.toBeVisible();
  await details.locator('summary').click();
  await expect(page.getByRole('list', { name: '下载中的文件' })).toBeVisible();
  await expect(details).toContainText('client.jar');
  await expect(details).toContainText('官方');
  await expect(details).toContainText('BMCLAPI');
  await page.setViewportSize({ width: 560, height: 800 });
  await page.screenshot({ path: info.outputPath('download-details-560.png'), fullPage: true, animations: 'disabled' });
});
