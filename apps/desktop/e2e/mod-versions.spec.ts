// SPDX-License-Identifier: MIT
// Synthetic IPC boundary only; no Modrinth requests or game execution occur.
import { test, expect, type Page } from '@playwright/test';
import type { Game, LocalMod, ModInstallPreview, ModVersion, ModVersionDetails, ModUpdatePreview } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type Hit = { project_id: string; slug: string; title: string; description: string; author: string; downloads: number };
type Managed = { project_id: string; version_id: string; filename: string; enabled: boolean; name: string };
type Mock = Window & {
  __calls: Call[];
  __fail: string;
  __withUpdates: boolean;
  __finishInstall: () => void;
  __emitGame: (game: Game) => void;
};
const game: Game = { id: 'versioned-forest', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
const hit: Hit = { project_id: 'forest-map-project', slug: 'forest-map', title: 'Forest Map', description: 'A useful map', author: 'Forest', downloads: 12000 };
const noVersionsHit: Hit = { ...hit, project_id: 'no-versions-project', title: 'Unsupported Mod' };
const stable: ModVersion = { version_id: 'map-v1-stable', name: 'Forest Map', version_number: '1.0.0', version_type: 'release', date_published: '2026-10-20T12:00:00Z' };
const beta: ModVersion = { version_id: 'map-v2-beta', name: 'Forest Map', version_number: '2.0.0-beta.1', version_type: 'beta', date_published: '2026-10-21T12:00:00Z' };
const older: ModVersion = { version_id: 'map-v0-old', name: 'Forest Map', version_number: '0.9.0', version_type: 'release', date_published: '2026-09-01T12:00:00Z' };
const maliciousDetails: ModVersionDetails = { ...beta, changelog: '<img src=x onerror=alert(1)>\n\n- Fixed trees' };
const emptyDetails: ModVersionDetails = { ...stable, changelog: '' };
const localMods: LocalMod[] = [
  { file_name: 'forest-map.jar.disabled', name: 'Forest Map', version: '1.0.0', authors: ['Forest'], loader: 'fabric', enabled: false, description: 'A useful map', pack_owned: false },
];
const managedMods: Managed[] = [{ project_id: hit.project_id, version_id: stable.version_id, filename: 'forest-map.jar', enabled: false, name: hit.title }];
const installPreview: ModInstallPreview = {
  plan_id: 'version-plan-beta', project_id: hit.project_id, version_id: beta.version_id,
  mods: [
    { project_id: hit.project_id, version_id: beta.version_id, name: hit.title, version_number: beta.version_number, required_by: null },
    { project_id: 'fabric-api-project', version_id: 'fabric-api-v9', name: 'Fabric API', version_number: '0.100.0', required_by: hit.title },
  ],
};
const changePreview: ModUpdatePreview = {
  plan_id: 'version-change-plan',
  changes: [{ project_id: hit.project_id, name: hit.title, from_version: stable.version_number, to_version: older.version_number, filename: 'forest-map.jar', enabled: false }],
};

async function setup(page: Page) {
  await page.addInitScript(({ game, hit, noVersionsHit, stable, beta, older, maliciousDetails, emptyDetails, localMods, managedMods, installPreview, changePreview }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__fail = ''; target.__withUpdates = false;
    let currentGame = game;
    const listeners = new Map<string, number>();
    let installed = [...localMods];
    let finishInstall: () => void = () => {};
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__emitGame = (next) => { currentGame = next; callbacks.get(listeners.get('game-install') ?? -1)?.({ event: 'game-install', id: 1, payload: next }); };
    target.__finishInstall = () => finishInstall();
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (target.__fail === command) throw new Error('synthetic Mod version failure');
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), args.handler as number); return 1; }
        if (command === 'list_games') return [currentGame];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Player', uuid: 'synthetic', note: null }], active_id: 'test-player' };
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_game_mods') return installed;
        if (command === 'search_game_mods') {
          const query = String(args.query || 'map');
          const result = query === 'no-versions' ? noVersionsHit : hit;
          return { hits: [{ ...result, project_id: query === 'slow' ? 'slow-project' : result.project_id, title: query === 'slow' ? 'Slow Mod' : query === 'fast' ? 'Fast Mod' : result.title }], offset: args.offset, total_hits: 1 };
        }
        if (command === 'list_game_mod_versions') {
          if (args.projectId === noVersionsHit.project_id) return [];
          if (args.projectId === 'slow-project') await new Promise((resolve) => setTimeout(resolve, 350));
          return [beta, stable, older];
        }
        if (command === 'get_game_mod_version_details') return args.versionId === stable.version_id ? emptyDetails : maliciousDetails;
        if (command === 'preview_game_mod_install') {
          const explicitId = String(args.versionId);
          return { ...installPreview, plan_id: `install-plan-${explicitId}`, version_id: explicitId, mods: installPreview.mods.map((mod) => mod.project_id === hit.project_id ? { ...mod, version_id: explicitId } : mod) };
        }
        if (command === 'install_game_mod_plan') {
          target.__emitGame({ ...currentGame, content_installing: true });
          await new Promise<void>((resolve) => { finishInstall = resolve; });
          installed = installed.map((mod) => mod.name === hit.title ? { ...mod, version: beta.version_number } : mod);
          target.__emitGame({ ...currentGame, content_installing: false });
          return { installed: 2, reused: 0 };
        }
        if (command === 'check_game_mod_updates') return { updates: target.__withUpdates ? [{ project_id: hit.project_id, name: hit.title, current_version: '1.0.0', version_number: '2.0.0-beta.1', version_id: beta.version_id, filename: 'forest-map.jar', enabled: false, pack_owned: false }] : [], managed_count: 1 };
        if (command === 'list_managed_game_mods') return managedMods;
        if (command === 'preview_game_mod_version_changes') return changePreview;
        if (command === 'apply_game_mod_updates') return { updated: 1, added: 0 };
        if (command === 'start_game') return { ...currentGame, launch_state: 'preparing' };
        return null;
      },
    };
  }, { game, hit, noVersionsHit, stable, beta, older, maliciousDetails, emptyDetails, localMods, managedMods, installPreview, changePreview });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
}

async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}
async function searchFor(page: Page, query = 'map') {
  await page.getByRole('tab', { name: '发现 Mod', exact: true }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill(query);
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  const title = query === 'no-versions' ? noVersionsHit.title : query === 'slow' ? 'Slow Mod' : query === 'fast' ? 'Fast Mod' : hit.title;
  await page.getByRole('button', { name: `查看 ${title}`, exact: true }).click();
  return title;
}

test('discovery prefers a stable default and previews the explicitly selected beta with its dependencies', async ({ page }, testInfo) => {
  await setup(page);
  await searchFor(page);
  await expect(page.getByLabel('Mod 版本')).toHaveValue(stable.version_id);
  expect((await calls(page, 'list_game_mod_versions'))[0].args).toEqual({ id: game.id, projectId: hit.project_id });
  await expect(page.getByText(`${hit.title} 的必需依赖`)).toBeVisible();
  expect((await calls(page, 'preview_game_mod_install'))[0].args).toEqual({ id: game.id, projectId: hit.project_id, versionId: stable.version_id });
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('mod-versions-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('mod-versions-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByLabel('Mod 版本').selectOption(beta.version_id);
  await expect(page.getByText(`${hit.title} 的必需依赖`)).toBeVisible();
  expect((await calls(page, 'preview_game_mod_install')).at(-1)?.args).toEqual({ id: game.id, projectId: hit.project_id, versionId: beta.version_id });
  await expect(page.getByRole('list', { name: '安装内容' })).toContainText('Fabric API');
  await expect(page.getByRole('list', { name: '安装内容' })).toContainText(beta.version_number);
});

test('changelog is fetched on request, escaped as text, and empty and failed details have useful states', async ({ page }, testInfo) => {
  await setup(page);
  await searchFor(page);
  expect(await calls(page, 'get_game_mod_version_details')).toHaveLength(0);
  await page.getByRole('button', { name: '查看更新日志', exact: true }).click();
  await expect(page.getByRole('region', { name: '更新日志' })).toContainText('此版本没有提供更新日志。');
  await page.getByLabel('Mod 版本').selectOption(beta.version_id);
  await page.getByRole('button', { name: '查看更新日志', exact: true }).click();
  await expect(page.getByRole('region', { name: '更新日志' })).toContainText('<img src=x onerror=alert(1)>');
  await expect(page.locator('[role="tabpanel"] img')).toHaveCount(0);
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('mod-changelog-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('mod-changelog-dark-560.png'), fullPage: true, animations: 'disabled' });
  expect((await calls(page, 'get_game_mod_version_details')).map((call) => call.args)).toEqual([
    { id: game.id, projectId: hit.project_id, versionId: stable.version_id },
    { id: game.id, projectId: hit.project_id, versionId: beta.version_id },
  ]);
  await page.getByLabel('Mod 版本').selectOption(stable.version_id);
  await page.getByLabel('Mod 版本').selectOption(beta.version_id);
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'get_game_mod_version_details'; });
  await page.getByRole('button', { name: '查看更新日志', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '重试读取日志', exact: true }).click();
  await expect(page.getByRole('region', { name: '更新日志' })).toContainText('Fixed trees');
});

test('unsupported projects show no compatible versions and version lookup can retry', async ({ page }) => {
  await setup(page);
  await searchFor(page, 'no-versions');
  await expect(page.getByText('没有适用于当前游戏和加载器的 Mod 版本。')).toBeVisible();
  await expect(page.getByRole('button', { name: '预览安装', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: '← 返回搜索结果' }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('map');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.evaluate(() => { (window as unknown as Mock).__fail = 'list_game_mod_versions'; });
  await page.getByRole('button', { name: `查看 ${hit.title}`, exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法读取 Mod 版本');
  await page.evaluate(() => { (window as unknown as Mock).__fail = ''; });
  await page.getByRole('button', { name: '重试读取版本', exact: true }).click();
  await expect(page.getByLabel('Mod 版本')).toHaveValue(stable.version_id);
});

test('late version lookup cannot replace the newest selected project', async ({ page }) => {
  await setup(page);
  await page.getByRole('tab', { name: '发现 Mod', exact: true }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('slow');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByRole('button', { name: '查看 Slow Mod', exact: true }).click();
  await page.getByRole('button', { name: '← 返回搜索结果' }).click();
  await page.getByRole('searchbox', { name: '搜索 Modrinth' }).fill('fast');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByRole('button', { name: '查看 Fast Mod', exact: true }).click();
  await expect(page.getByLabel('Mod 版本')).toHaveValue(stable.version_id);
  await page.waitForTimeout(400);
  expect((await calls(page, 'list_game_mod_versions')).map((call) => call.args.projectId)).toEqual(['slow-project', hit.project_id]);
  await expect(page.getByRole('heading', { name: 'Fast Mod', exact: true })).toBeVisible();
});

test('version management loads only on demand and previews an older version without enabling a disabled Mod', async ({ page }, testInfo) => {
  await setup(page);
  await page.getByRole('tab', { name: '更新', exact: true }).click();
  expect(await calls(page, 'list_managed_game_mods')).toHaveLength(0);
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await expect(page.getByText('已检查的 Modrinth Mod 暂无可用更新。')).toBeVisible();
  await page.getByRole('button', { name: '版本管理', exact: true }).click();
  await expect(page.getByRole('list', { name: '可管理版本的 Mod' })).toContainText('forest-map.jar');
  expect((await calls(page, 'list_managed_game_mods'))[0].args).toEqual({ id: game.id });
  await expect(page.getByRole('list', { name: '可管理版本的 Mod' })).toContainText('已停用');
  await page.getByRole('button', { name: '管理版本 Forest Map', exact: true }).click();
  await expect(page.getByLabel('Mod 版本')).toHaveValue(stable.version_id);
  await page.getByLabel('Mod 版本').selectOption(older.version_id);
  await page.getByRole('button', { name: '预览版本变更', exact: true }).click();
  await expect(page.getByRole('heading', { name: /^更换版本/ })).toBeVisible();
  expect((await calls(page, 'preview_game_mod_version_changes')).at(-1)?.args).toEqual({ id: game.id, selections: [{ project_id: hit.project_id, version_id: older.version_id }] });
  await expect(page.getByRole('list', { name: '更新内容' })).toContainText('保持停用');
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('managed-version-change-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('managed-version-change-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '确认更换', exact: true }).click();
  expect((await calls(page, 'apply_game_mod_updates')).at(-1)?.args).toEqual({ id: game.id, planId: changePreview.plan_id });
  await expect(page.getByRole('status')).toContainText('版本已更换。');
  expect(await calls(page, 'list_managed_game_mods')).toHaveLength(2);
});

test('the update row can open version and changelog management for its preferred version', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => { (window as unknown as Mock).__withUpdates = true; });
  await page.getByRole('tab', { name: '更新', exact: true }).click();
  await page.getByRole('button', { name: '检查更新', exact: true }).click();
  await page.getByRole('button', { name: `版本与日志 ${hit.title}`, exact: true }).click();
  await expect(page.getByLabel('Mod 版本')).toHaveValue(beta.version_id);
  expect((await calls(page, 'list_managed_game_mods')).at(-1)?.args).toEqual({ id: game.id });
});
