// SPDX-License-Identifier: MIT
// Synthetic native boundary only. Actual Provider/HTTP/Viewer proof is separate.
import { expect, test } from '@playwright/test';
import type { Game } from '../src/games';
import type { OnlineBlueprint } from '../src/online-blueprints';
import { blueprintThumbnail as thumbnail } from './fixtures/blueprint-thumbnail';

type Call = { command: string; args: Record<string, unknown> };
type ProviderWindow = Window & { __providerCalls: Call[]; __enableProvider: () => void };
const game: Game = { id: 'blueprint-browser-game', name: '蓝图测试', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'idle' };
const item: OnlineBlueprint = { id: '11111111-1111-4111-8111-111111111111', title: '测试盆栽', description: '本地界面测试数据。', version: 1, format: 'schem', author: '测试作者', attribution: 'repost', license: null, java_release: '1.21.11', namespaces: ['minecraft'], preview_url: null };
test('online blueprints require the installed provider and reuse preview, save and import at narrow width', async ({ page }, testInfo) => {
  await page.addInitScript(({ initialGame, resource, png }) => {
    const target = window as unknown as ProviderWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__providerCalls = []; localStorage.setItem('cobble-language', 'zh-CN');
    let enabled = false; let saved = false; let favorite = false;
    const libraryItem = () => ({ id: 'saved-online', name: resource.title, pluginId: 'schematic-viewer', format: 'schem', favorite, hasThumbnail: true, source: { provider: 'community-provider', id: resource.id, version: resource.version, author: resource.author, attribution: resource.attribution, license: resource.license } });
    const otherItem = { id: 'other-local', name: '山间小屋', pluginId: 'schematic-viewer', format: 'litematic' };
    target.__enableProvider = () => { enabled = true; };
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__providerCalls.push({ command, args });
        if (command === 'plugin:event|listen') return 1;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'get_blueprint_viewer_choices') return { viewer_id: 'synthetic-viewer', viewers: [{ id: 'synthetic-viewer', name: 'Fixture Viewer', file_extensions: ['schem', 'litematic'] }], disabled_viewers: [], interface: { id: 'launcher:viewer', version: '^1.0.0', implementations: [{ plugin_id: 'synthetic-viewer', name: 'Fixture Viewer', version: '1.0.0' }], candidates: [] } };
        if (command === 'list_games') return [initialGame];
        if (command === 'list_tasks' || command === 'list_plugins' || command === 'list_extension_contributions') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'list_blueprints') return saved ? [otherItem, libraryItem()] : [otherItem];
        if (command === 'set_blueprint_favorite') { favorite = Boolean(args.favorite); return [otherItem, libraryItem()]; }
        if (command === 'read_blueprint_thumbnail') return args.id === 'saved-online' ? png : null;
        if (command === 'save_blueprint_thumbnail') return null;
        if (command === 'get_blueprint_provider_availability') return { available: enabled, provider_id: null, providers: enabled ? [{ id: 'mcs-provider', name: 'MCS', origins: ['https://www.mcschematic.top'] }, { id: 'community-provider', name: 'Community', origins: ['https://community.example'] }] : [], viewer_available: true };
        if (command === 'search_online_blueprints') return { items: [resource], next_cursor: null, detail: null, download: null };
        if (command === 'online_blueprint_details') return { items: [], next_cursor: null, detail: resource, download: null };
        if (command === 'download_online_blueprint') { saved = true; return libraryItem(); }
        if (command === 'open_blueprint' || command === 'open_online_blueprint') return { resourceId: 'opaque-online-resource', name: resource.title, format: 'schem', scene: { width: 1, height: 1, length: 1, palette: ['minecraft:stone'], materials: [{ state: 'minecraft:stone', count: 1 }, { state: 'minecraft:dirt', count: 12 }], chunk_count: 1 } };
        if (command === 'read_viewer_chunk') return { blocks: [{ x: 0, y: 0, z: 0, palette_index: 0 }] };
        if (command === 'save_blueprint') { saved = true; return libraryItem(); }
        if (command === 'list_game_mods') return [];
        if (command === 'import_blueprint') return { destination: 'Synthetic/schematics/planter.schem' };
        return null;
      },
    };
  }, { initialGame: game, resource: item, png: thumbnail });
  await page.setViewportSize({ width: 560, height: 860 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.getByRole('button', { name: '工具', exact: true }).click();
  await page.locator('.tool-card').filter({hasText:'蓝图库'}).click();
  await page.getByRole('searchbox', { name: '搜索已保存的蓝图', exact: true }).fill('does-not-match');
  await page.getByRole('button', { name: '收藏', exact: true }).click();
  await page.getByRole('tab', { name: '在线蓝图', exact: true }).click();
  await expect(page.getByText('请先安装并启用蓝图来源插件。', { exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as ProviderWindow).__providerCalls.some(call => call.command === 'search_online_blueprints'))).toBe(false);
  await page.evaluate(() => (window as unknown as ProviderWindow).__enableProvider());
  await page.locator('.online-blueprints').getByRole('button', { name: '重试', exact: true }).click();
  await expect(page.locator('.online-blueprints').getByRole('button', { name: '搜索', exact: true })).toBeDisabled();
  await page.getByRole('combobox', { name: '来源', exact: true }).selectOption('community-provider');
  await page.getByRole('searchbox', { name: '搜索蓝图', exact: true }).fill('盆栽');
  await page.locator('.online-blueprints').getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('button', { name: item.title, exact: true })).toBeVisible();
  await page.getByRole('button', { name: item.title, exact: true }).click();
  await expect(page.getByRole('heading', { name: item.title, exact: true })).toBeVisible();
  await expect(page.locator('.online-detail')).toContainText('转载 · 许可未提供');
  await expect(page.locator('.online-blueprints')).toContainText('1.21.11');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  const screenshot = testInfo.outputPath('online-blueprints-560.png');
  await page.screenshot({ path: screenshot, fullPage: true });
  await testInfo.attach('online-blueprints-560', { path: screenshot, contentType: 'image/png' });
  await page.getByRole('button', { name: '保存到资料库', exact: true }).click();
  await expect(page.getByRole('tab', { name: '已保存', exact: true })).toHaveAttribute('aria-selected', 'true');
  const card = page.locator('[data-blueprint-id="saved-online"]');
  await expect(card).toHaveClass(/blueprint-located/);
  await expect(card.locator('.blueprint-open')).toBeFocused();
  await expect(page.locator('#blueprint-library-search')).toHaveValue('');
  await expect(page.getByRole('button', { name: '收藏', exact: true })).toHaveAttribute('aria-pressed', 'false');
  await expect(card).toContainText('测试作者 · community-provider');
  await expect(card.locator('img')).toHaveAttribute('src', thumbnail);
  await expect.poll(() => card.locator('img').evaluate(image => (image as HTMLImageElement).naturalWidth)).toBe(320);
  await expect(card).toContainText('转载');
  await card.getByRole('button', { name: '收藏蓝图', exact: true }).click();
  await expect(card.getByRole('button', { name: '取消收藏', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: '收藏', exact: true }).click();
  await expect(page.locator('.blueprint-library > li')).toHaveCount(1);
  await page.locator('#blueprint-library-search').fill('测试作者');
  await expect(card).toBeVisible();
  await page.locator('#blueprint-library-search').fill('no-match');
  await expect(page.locator('.blueprint-library > li')).toHaveCount(0);
  await page.locator('#blueprint-library-search').fill('');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  const libraryScreenshot = testInfo.outputPath('blueprint-library-560.png');
  await page.screenshot({ path: libraryScreenshot, fullPage: true });
  await testInfo.attach('blueprint-library-560', { path: libraryScreenshot, contentType: 'image/png' });
  await card.locator('.blueprint-open').press('Enter');
  await expect(page.locator('.blueprint-viewer canvas')).toBeVisible();
  await expect(page.locator('.blueprint-library')).toContainText(item.title);
  await expect(page.locator('.blueprint-viewer table')).toContainText('minecraft:stone');
  await page.getByRole('button', { name: '导入游戏', exact: true }).click();
  await page.getByRole('combobox', { name: '选择游戏', exact: true }).click();
  await page.getByRole('option', { name: game.name, exact: true }).click();
  await expect(page.locator('.blueprint-import')).toContainText('未检测到启用的 WorldEdit');
  await expect(page.getByRole('combobox', { name: '文件重名时', exact: true })).toContainText('停止并保留原文件');
  await page.getByRole('combobox', { name: '文件重名时', exact: true }).click();
  await page.getByRole('option', { name: '以新名称保留两份', exact: true }).click();
  await expect(page.locator('.blueprint-viewer tbody tr').first()).toContainText('minecraft:dirt');
  await page.getByRole('combobox', { name: '材料排序', exact: true }).click();
  await page.getByRole('option', { name: '材料名称', exact: true }).click();
  await page.getByRole('searchbox', { name: '搜索材料', exact: true }).fill('stone');
  await page.getByRole('button', { name: '复制显示的材料', exact: true }).click();
  await expect(page.getByRole('button', { name: '已复制', exact: true })).toBeVisible();
  await page.getByRole('searchbox', { name: '搜索材料', exact: true }).fill('');
  await page.getByRole('combobox', { name: '材料排序', exact: true }).click();
  await page.getByRole('option', { name: '数量从多到少', exact: true }).click();
  for (const width of [560, 1100]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(() => scrollTo(0, 0));
    await page.screenshot({ path: testInfo.outputPath(`blueprint-workflow-${width}.png`), fullPage: true, animations: 'disabled' });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.getByRole('button', { name: '导入 .schem', exact: true }).click();
  await expect(page.locator('.blueprint-import [role="status"]')).toContainText('Synthetic/schematics/planter.schem');
  const calls = await page.evaluate(() => (window as unknown as ProviderWindow).__providerCalls);
  expect(calls.find(call => call.command === 'search_online_blueprints')?.args).toMatchObject({ providerId: 'community-provider', gameId: game.id, query: '盆栽' });
  expect(calls.find(call => call.command === 'download_online_blueprint')?.args).toMatchObject({providerId:'community-provider',id:item.id,version:item.version});
  expect(calls.find(call => call.command === 'set_blueprint_favorite')?.args).toEqual({ id: 'saved-online', favorite: true });
  expect(calls.find(call => call.command === 'read_blueprint_thumbnail')?.args).toEqual({ id: 'saved-online' });
  expect(calls.find(call => call.command === 'open_blueprint')?.args).toMatchObject({ id: 'saved-online', pluginId: 'synthetic-viewer' });
  expect(calls.find(call => call.command === 'import_blueprint')?.args).toEqual({ id: game.id, blueprintId: 'saved-online', duplicatePolicy: 'keep_both' });
  expect(calls.find(call => call.command === 'plugin:clipboard-manager|write_text')?.args).toMatchObject({ text: '1\tminecraft:stone' });
  await page.getByRole('button', { name: '关闭预览', exact: true }).click();
  await expect(page.locator('.blueprint-viewer')).toHaveCount(0);
});



