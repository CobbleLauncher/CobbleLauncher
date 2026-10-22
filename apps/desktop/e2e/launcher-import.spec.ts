// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only; these tests do not inspect or import user game files.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type ContentRoot = { content_id: string; label: string; has_saves: boolean };
type Candidate = {
  candidate_id: string;
  name: string;
  minecraft_version: string;
  loader_kind: string | null;
  loader_version: string | null;
  supported: boolean;
  unsupported_reason: string | null;
  content_roots: ContentRoot[];
  warnings: string[];
};
type ImportPreview = { preview_id: string; source_display: string; candidates: Candidate[] };
type Mock = Window & {
  __calls: Call[];
  __preview: ImportPreview;
  __failCommand: string;
  __cancelNextPreview: () => void;
  __blockNextCreate: () => void;
  __finishCreate: () => void;
};
const game: Game = { id: 'launcher-imported-game', name: '迁移存档', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installing', error: null, progress: null };
const candidate: Candidate = {
  candidate_id: 'candidate-supported-fabric', name: '林间实例', minecraft_version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', supported: true, unsupported_reason: null,
  content_roots: [
    { content_id: 'root-with-saves', label: '.minecraft', has_saves: true },
    { content_id: 'root-instance-only', label: 'instances/林间实例', has_saves: false },
  ], warnings: [],
};
const unsupportedCandidate: Candidate = {
  candidate_id: 'candidate-unsupported', name: '旧版 Forge', minecraft_version: '1.12.2', loader_kind: 'forge', loader_version: '14.23.5.2860', supported: false, unsupported_reason: '此版本暂不支持导入。',
  content_roots: [{ content_id: 'unsupported-root', label: 'versions/1.12.2-forge', has_saves: false }], warnings: [],
};
const preview: ImportPreview = { preview_id: 'opaque-launcher-preview', source_display: 'C:\\Synthetic\\.minecraft', candidates: [candidate, unsupportedCandidate] };

async function setup(page: Page, source: 'automatic' | 'official' = 'automatic') {
  await page.addInitScript(({ game, preview, source }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__preview = structuredClone(preview); target.__failCommand = '';
    let cancelNextPreview = false;
    let createBlocked = false;
    let finishCreate: () => void = () => {};
    target.__cancelNextPreview = () => { cancelNextPreview = true; };
    target.__blockNextCreate = () => { createBlocked = true; };
    target.__finishCreate = () => { createBlocked = false; finishCreate(); };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (target.__failCommand === command) throw new Error('synthetic launcher import failure');
        if (command === 'plugin:event|listen') return 1;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_download_settings') return { source, mirror: '' };
        if (command === 'get_launcher_preferences') return { memory_mb: 2048, game_directory: 'C:\\Cobble\\games', custom_game_directory: false };
        if (command === 'default_game_directory') return 'C:\\Cobble\\games';
        if (command === 'preview_launcher_import') {
          if (cancelNextPreview) { cancelNextPreview = false; return null; }
          return structuredClone(target.__preview);
        }
        if (command === 'pick_game_directory') return 'D:\\Imported Games';
        if (command === 'create_launcher_import') {
          if (createBlocked) await new Promise<void>((resolve) => { finishCreate = resolve; });
          return { ...game, name: args.name };
        }
        return null;
      },
    };
  }, { game, preview, source });
  await page.goto('/');
}
async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}
async function openImport(page: Page) {
  await page.getByRole('button', { name: '导入已有游戏', exact: true }).click();
  await expect(page.getByRole('heading', { name: '导入已有游戏', exact: true })).toBeVisible();
}
async function loadPreview(page: Page) {
  const picker = page.getByRole('button', { name: /选择游戏目录/ });
  await expect(picker).toBeEnabled();
  await picker.click();
  await expect(page.getByLabel('来源游戏')).toBeVisible();
}
async function chooseSupportedRoot(page: Page) {
  await page.getByLabel('来源游戏').selectOption(candidate.candidate_id);
  await page.getByLabel('游戏数据目录').selectOption('root-with-saves');
}

test('source selection is explicit; unsupported games and ambiguous roots cannot be submitted', async ({ page }) => {
  await setup(page);
  expect(await calls(page, 'preview_launcher_import')).toHaveLength(0);
  expect(await calls(page, 'create_launcher_import')).toHaveLength(0);
  await openImport(page);
  await expect(page.getByRole('heading', { name: '导入已有游戏', exact: true })).toBeVisible();
  expect(await calls(page, 'preview_launcher_import')).toHaveLength(0);
  await loadPreview(page);
  expect((await calls(page, 'preview_launcher_import'))[0].args).toEqual({});
  await expect(page.getByLabel('来源游戏')).toHaveValue('');
  await page.getByLabel('来源游戏').selectOption(unsupportedCandidate.candidate_id);
  await expect(page.getByText(unsupportedCandidate.unsupported_reason!, { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '导入为独立游戏', exact: true })).toBeDisabled();
  await page.getByLabel('来源游戏').selectOption(candidate.candidate_id);
  await expect(page.getByLabel('游戏数据目录')).toHaveValue('');
  await expect(page.getByRole('button', { name: '导入为独立游戏', exact: true })).toBeDisabled();
  await page.getByLabel('游戏数据目录').selectOption('root-with-saves');
  await expect(page.getByLabel('包含存档')).not.toBeChecked();
});

test('native picker cancellation preserves preview and edits; create errors keep the chosen source retryable', async ({ page }, testInfo) => {
  await setup(page);
  await openImport(page);
  await loadPreview(page);
  await chooseSupportedRoot(page);
  await page.getByLabel('游戏名称').fill('迁移后的林间实例');
  await expect(page.getByRole('radio', { name: '自动（推荐）', exact: true })).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('button', { name: /C:\\Cobble\\games.*更改/ }).click();
  await expect(page.getByRole('button', { name: /D:\\Imported Games.*更改/ })).toBeVisible();
  expect((await calls(page, 'pick_game_directory'))[0].args).toEqual({});
  await page.evaluate(() => (window as unknown as Mock).__cancelNextPreview());
  await page.getByRole('button', { name: /更换来源目录/ }).click();
  await expect(page.getByLabel('游戏名称')).toHaveValue('迁移后的林间实例');
  await expect(page.getByLabel('来源游戏')).toHaveValue(candidate.candidate_id);
  await expect(page.getByLabel('游戏数据目录')).toHaveValue('root-with-saves');
  await expect(page.getByLabel('包含存档')).not.toBeChecked();
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('launcher-import-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('launcher-import-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'create_launcher_import'; });
  await page.getByRole('button', { name: '导入为独立游戏', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法导入游戏');
  await expect(page.getByLabel('游戏名称')).toHaveValue('迁移后的林间实例');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '导入为独立游戏', exact: true }).click();
  await expect(page.getByRole('article', { name: '迁移后的林间实例' })).toBeVisible();
  expect((await calls(page, 'create_launcher_import')).map((call) => call.args)).toEqual([
    { previewId: preview.preview_id, candidateId: candidate.candidate_id, contentId: 'root-with-saves', name: '迁移后的林间实例', includeSaves: false, source: { source: 'automatic', mirror: '' }, useDefaultDirectory: false },
    { previewId: preview.preview_id, candidateId: candidate.candidate_id, contentId: 'root-with-saves', name: '迁移后的林间实例', includeSaves: false, source: { source: 'automatic', mirror: '' }, useDefaultDirectory: false },
  ]);
});

test('preview errors retry through the native directory picker without fabricating a preview', async ({ page }) => {
  await setup(page);
  await openImport(page);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'preview_launcher_import'; });
  await page.getByRole('button', { name: /选择游戏目录/ }).click();
  await expect(page.getByRole('alert')).toContainText('无法读取来源游戏');
  await expect(page.getByRole('button', { name: '导入为独立游戏', exact: true })).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: /选择游戏目录/ }).click();
  await expect(page.getByLabel('来源游戏')).toBeVisible();
  expect(await calls(page, 'preview_launcher_import')).toHaveLength(2);
  expect(await calls(page, 'create_launcher_import')).toHaveLength(0);
});

test('creation locks navigation while native installation is pending', async ({ page }) => {
  await setup(page);
  await openImport(page);
  await loadPreview(page);
  await chooseSupportedRoot(page);
  await page.getByLabel('游戏名称').fill('正在迁移');
  await page.evaluate(() => (window as unknown as Mock).__blockNextCreate());
  await page.getByRole('button', { name: '导入为独立游戏', exact: true }).click();
  await expect(page.getByRole('button', { name: '正在创建…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '返回游戏库' })).toBeDisabled();
  await expect(page.getByLabel('游戏名称')).toBeDisabled();
  await expect.poll(async () => (await calls(page, 'create_launcher_import')).length).toBe(1);
  await page.evaluate(() => (window as unknown as Mock).__finishCreate());
  await expect(page.getByRole('article', { name: '正在迁移' })).toBeVisible();
});

test('an existing Official download-source choice is retained in the import request', async ({ page }) => {
  await setup(page, 'official');
  await openImport(page);
  await loadPreview(page);
  await chooseSupportedRoot(page);
  await page.getByLabel('游戏名称').fill('保留官方源');
  await expect(page.getByRole('radio', { name: '官方', exact: true })).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('button', { name: '导入为独立游戏', exact: true }).click();
  const args = (await calls(page, 'create_launcher_import'))[0].args;
  expect(args).toMatchObject({ source: { source: 'official', mirror: '' }, useDefaultDirectory: true });
});
