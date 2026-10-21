// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only; these tests do not access local ZIP files.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type Pack = { file_name: string; name: string; description: string | null; size_bytes: number; pack_format: number | null };
type Mock = Window & {
  __calls: Call[];
  __cancelPackImport: boolean;
  __failCommand: string;
  __blockNextPackImport: () => void;
  __finishPackImport: () => void;
  __emitGame: (game: Game) => void;
};
const game: Game = { id: 'forest', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
const resourcePack: Pack = { file_name: 'leaves.zip', name: '落叶纹理', description: '柔和的林地色彩', size_bytes: 4096, pack_format: 15 };
const shaderPack: Pack = { file_name: 'soft-light.zip', name: '柔光', description: null, size_bytes: 8192, pack_format: null };

async function setup(page: Page, initial: { resource?: Pack[]; shader?: Pack[] } = {}) {
  await page.addInitScript(({ game, resourcePack, shaderPack, initial }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__cancelPackImport = false; target.__failCommand = '';
    let listener = 0;
    let runningGame = game;
    let resourcePacks = [...(initial.resource ?? [])];
    let shaderPacks = [...(initial.shader ?? [])];
    let blockImport = false;
    let finishImport: () => void = () => {};
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__emitGame = (value) => {
      runningGame = value;
      callbacks.get(listener)?.({ event: 'game-install', id: 1, payload: value });
    };
    target.__blockNextPackImport = () => { blockImport = true; };
    target.__finishPackImport = () => finishImport();
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === target.__failCommand) throw new Error('synthetic pack failure');
        if (command === 'plugin:event|listen') { listener = args.handler as number; return 1; }
        if (command === 'list_games') return [runningGame];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Player', uuid: 'synthetic', note: null }], active_id: 'test-player' };
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_game_mods') return [];
        if (command === 'list_game_packs') return args.kind === 'resource_pack' ? resourcePacks : shaderPacks;
        if (command === 'import_game_packs') {
          if (target.__cancelPackImport) return null;
          if (blockImport) await new Promise<void>((resolve) => { finishImport = resolve; });
          if (args.kind === 'resource_pack') resourcePacks = [resourcePack];
          else shaderPacks = [shaderPack];
          return args.kind === 'resource_pack' ? resourcePacks : shaderPacks;
        }
        if (command === 'remove_game_pack') {
          if (args.kind === 'resource_pack') resourcePacks = resourcePacks.filter((pack) => pack.file_name !== args.fileName);
          else shaderPacks = shaderPacks.filter((pack) => pack.file_name !== args.fileName);
          return args.kind === 'resource_pack' ? resourcePacks : shaderPacks;
        }
        if (command === 'open_game_pack_directory') return null;
        if (command === 'start_game') return { ...runningGame, launch_state: 'preparing' };
        return null;
      },
    };
  }, { game, resourcePack, shaderPack, initial });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
}
async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}
async function choosePackKind(page: Page, label: '资源包' | '光影包') {
  await page.getByRole('tab', { name: label, exact: true }).click();
}

test('resource and shader packs load lazily by kind and retain readable metadata', async ({ page }) => {
  await setup(page, { resource: [resourcePack], shader: [shaderPack] });
  expect(await calls(page, 'list_game_packs')).toHaveLength(0);
  await choosePackKind(page, '资源包');
  await expect(page.getByRole('heading', { name: /本地资源包/ })).toBeVisible();
  await expect(page.getByRole('list', { name: '本地资源包' })).toContainText('落叶纹理');
  await expect(page.getByText('柔和的林地色彩')).toBeVisible();
  await expect(page.getByRole('switch')).toHaveCount(0);
  expect((await calls(page, 'list_game_packs'))[0].args).toEqual({ id: game.id, kind: 'resource_pack' });
  await choosePackKind(page, '光影包');
  await expect(page.getByRole('list', { name: '本地光影包' })).toContainText('柔光');
  expect((await calls(page, 'list_game_packs'))[1].args).toEqual({ id: game.id, kind: 'shader_pack' });
  await choosePackKind(page, '资源包');
  expect(await calls(page, 'list_game_packs')).toHaveLength(2);
});

test('resource pack import cancellation preserves the list and successful import is visible at 560px', async ({ page }, testInfo) => {
  await setup(page);
  await choosePackKind(page, '资源包');
  await expect(page.getByText('还没有资源包，导入本地 ZIP 文件开始。')).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__cancelPackImport = true; });
  await page.getByRole('button', { name: '导入资源包', exact: true }).click();
  expect((await calls(page, 'import_game_packs')).at(-1)?.args).toEqual({ id: game.id, kind: 'resource_pack' });
  await expect(page.getByText(resourcePack.name, { exact: true })).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as Mock).__cancelPackImport = false; });
  await page.getByRole('button', { name: '导入资源包', exact: true }).click();
  await expect(page.getByRole('list', { name: '本地资源包' })).toContainText(resourcePack.name);
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('resource-packs-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('resource-packs-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '打开资源包文件夹', exact: true }).click();
  expect((await calls(page, 'open_game_pack_directory')).at(-1)?.args).toEqual({ id: game.id, kind: 'resource_pack' });
});

test('removal stays confirmed after failure and can be retried', async ({ page }) => {
  await setup(page, { shader: [shaderPack] });
  await choosePackKind(page, '光影包');
  await expect(page.getByRole('list', { name: '本地光影包' })).toContainText(shaderPack.name);
  await page.getByRole('button', { name: `移除 ${shaderPack.name}`, exact: true }).click();
  await expect(page.getByText('移除 柔光？只删除这个 ZIP 文件。')).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'remove_game_pack'; });
  await page.getByRole('button', { name: '确认移除', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByRole('list', { name: '本地光影包' })).toContainText(shaderPack.name);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '确认移除', exact: true }).click();
  await expect(page.getByText('还没有光影包，导入本地 ZIP 文件开始。')).toBeVisible();
  expect((await calls(page, 'remove_game_pack')).map((call) => call.args)).toEqual([
    { id: game.id, kind: 'shader_pack', fileName: shaderPack.file_name },
    { id: game.id, kind: 'shader_pack', fileName: shaderPack.file_name },
  ]);
});

test('pack import locks navigation and local changes; installed games remain readable while running', async ({ page }) => {
  await setup(page, { resource: [resourcePack] });
  await choosePackKind(page, '资源包');
  await page.evaluate(() => (window as unknown as Mock).__blockNextPackImport());
  await page.getByRole('button', { name: '导入资源包', exact: true }).click();
  await expect(page.getByRole('button', { name: '导入中…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '处理内容中…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '返回游戏库' })).toBeDisabled();
  await expect(page.getByRole('tab', { name: '光影包', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: `移除 ${resourcePack.name}`, exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '导入中…', exact: true })).toBeDisabled();
  await page.evaluate(() => (window as unknown as Mock).__finishPackImport());
  await expect(page.getByRole('button', { name: `移除 ${resourcePack.name}`, exact: true })).toBeEnabled();
  await page.evaluate((value) => (window as unknown as Mock).__emitGame({ ...value, launch_state: 'running' }), game);
  await expect(page.getByRole('button', { name: `移除 ${resourcePack.name}`, exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '打开资源包文件夹', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: '导入资源包', exact: true })).toBeDisabled();
  await choosePackKind(page, '光影包');
  await expect(page.getByText('还没有光影包，导入本地 ZIP 文件开始。')).toBeVisible();
  expect((await calls(page, 'list_game_packs'))[1].args).toEqual({ id: game.id, kind: 'shader_pack' });
});
