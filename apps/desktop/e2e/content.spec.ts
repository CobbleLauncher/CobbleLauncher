// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only; these tests do not install or execute game files.
import { test, expect, type Page } from '@playwright/test';
import type { Game, LocalMod } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type MockWindow = Window & { __calls: Call[]; __fabricMode: 'normal' | 'error' | 'race'; __cancelImport: boolean; __actionFails: boolean; __emitGame: (game: Game) => void };
const game: Game = { id: 'local-game', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
const mod: LocalMod = { file_name: 'synthetic.jar', name: '林间地图', version: '1.2.3', authors: ['Cobble'], loader: 'fabric', enabled: true, description: '原创测试模组' };
async function mock(page: Page, stored: Game[] = [], initialMods: LocalMod[] = []) {
  await page.addInitScript(({ stored, game, mod, initialMods }) => {
    const target = window as unknown as MockWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true;
    target.__calls = [];
    target.__fabricMode = 'normal';
    target.__cancelImport = false;
    target.__actionFails = false;
    const callbacks = new Map<number, (event: unknown) => void>();
    let listener = 0;
    let mods: LocalMod[] = initialMods;
    target.__emitGame = (value) => callbacks.get(listener)?.({ event: 'game-install', id: 1, payload: value });
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') { listener = args.handler as number; return 1; }
        if (command === 'list_games') return stored;
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Cobble', uuid: 'synthetic', note: null }], active_id: 'test-player' };
        if (command === 'get_download_settings') return { source: 'bmclapi', mirror: '' };
        if (command === 'default_game_directory') return 'synthetic default';
        if (command === 'list_game_versions') return [{ id: '1.21.1', kind: 'release' }, { id: '1.20.6', kind: 'release' }];
        if (command === 'list_fabric_versions') {
          if (target.__fabricMode === 'error') throw new Error('网络不可用');
          if (target.__fabricMode === 'race' && args.version === '1.21.1') {
            await new Promise((resolve) => setTimeout(resolve, 600));
            return [{ version: 'stale', stable: true }];
          }
          return args.version === '1.20.6' ? [{ version: '0.15.11', stable: true }] : [{ version: '0.17.0-beta', stable: false }, { version: '0.16.14', stable: true }];
        }
        if (command === 'list_loader_versions') {
          if (args.version !== '1.21.1') throw new Error('暂不支持此 Minecraft 版本，目前支持 1.21.1');
          return args.kind === 'forge' ? [{ version: '52.1.0', recommended: false }, { version: '52.0.28', recommended: true }] : [{ version: '21.1.100', recommended: false }, { version: '21.1.99', recommended: false }];
        }
        if (command === 'create_game') return { ...game, name: args.name, version: args.version, loader_kind: args.loaderKind, loader_version: args.loaderVersion };
        if (command === 'list_game_mods') return mods;
        if (command === 'rename_game') {
          if (target.__actionFails) throw new Error('游戏记录暂时不可用');
          return { ...(stored[0] ?? game), name: args.name };
        }
        if (command === 'open_game_directory' || command === 'open_game_log') {
          if (target.__actionFails) throw new Error('文件已不存在');
          return null;
        }
        if (command === 'import_game_mods') { if (target.__cancelImport) return null; mods = [mod]; return mods; }
        if (command === 'set_game_mod_enabled') { mods = mods.map((item) => ({ ...item, enabled: args.enabled as boolean, file_name: args.enabled ? 'synthetic.jar' : 'synthetic.jar.disabled' })); return mods; }
        if (command === 'remove_game_mod') { mods = []; return mods; }
        if (command === 'start_game') return { ...game, launch_state: 'preparing' };
        if (command === 'plugin:window|is_maximized') return false;
        return null;
      },
    };
  }, { stored, game, mod, initialMods });
}

test('Fabric chooses stable loader and submits selected version and source', async ({ page }) => {
  await mock(page);
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await expect(page.getByLabel('Minecraft 版本')).toHaveValue('1.21.1');
  await page.getByRole('radio', { name: 'Fabric', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.16.14');
  await page.getByRole('button', { name: '创建并安装' }).click();
  await expect(page.getByRole('article')).toContainText('Fabric 0.16.14');
  const calls = await page.evaluate(() => (window as unknown as MockWindow).__calls);
  expect(calls.find((call) => call.command === 'list_fabric_versions')?.args).toEqual({ version: '1.21.1', source: { source: 'bmclapi', mirror: '' } });
  expect(calls.find((call) => call.command === 'create_game')?.args.loaderVersion).toBe('0.16.14');
  expect(calls.find((call) => call.command === 'create_game')?.args.loaderKind).toBe('fabric');
});

test('Fabric load failure can retry and return to Vanilla', async ({ page }) => {
  await mock(page);
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await page.evaluate(() => { (window as unknown as MockWindow).__fabricMode = 'error'; });
  await page.getByRole('radio', { name: 'Fabric', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('无法获取 Fabric 版本');
  await expect(page.getByRole('button', { name: '创建并安装' })).toBeDisabled();
  await page.evaluate(() => { (window as unknown as MockWindow).__fabricMode = 'normal'; });
  await page.getByRole('button', { name: '重试', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.16.14');
  await page.getByRole('radio', { name: '原版', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toHaveCount(0);
  await page.getByRole('button', { name: '创建并安装' }).click();
  expect(await page.evaluate(() => (window as unknown as MockWindow).__calls.find((call) => call.command === 'create_game')?.args.loaderVersion)).toBeNull();
});

test('late Fabric response cannot overwrite a newly selected Minecraft version', async ({ page }) => {
  await mock(page);
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await page.evaluate(() => { (window as unknown as MockWindow).__fabricMode = 'race'; });
  await page.getByRole('radio', { name: 'Fabric', exact: true }).click();
  await expect(page.getByLabel('Fabric 版本')).toBeDisabled();
  await page.getByLabel('Minecraft 版本').selectOption('1.20.6');
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.15.11');
  await expect(page.getByLabel('Fabric 版本').getByRole('option', { name: 'stale' })).toHaveCount(0);
  await page.waitForTimeout(700);
  await expect(page.getByLabel('Fabric 版本')).toHaveValue('0.15.11');
});

test('manage local Mods imports, searches, toggles, refreshes and confirms deletion', async ({ page }, testInfo) => {
  await mock(page, [game]);
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
  await expect(page.getByRole('heading', { name: game.name })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByText('还没有 Mod，导入本地 .jar 文件开始。')).toBeVisible();
  await page.evaluate(() => { (window as unknown as MockWindow).__cancelImport = true; });
  await page.getByRole('button', { name: '导入 Mod' }).click();
  await expect(page.getByRole('switch')).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as MockWindow).__cancelImport = false; });
  await page.getByRole('button', { name: '导入 Mod' }).click();
  await expect(page.getByRole('heading', { name: mod.name })).toBeVisible();
  await page.getByRole('searchbox').fill('not-found');
  await expect(page.getByText('没有找到匹配的 Mod。')).toBeVisible();
  await page.getByRole('searchbox').fill('Cobble');
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('switch').click();
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  await expect(page.getByText('synthetic.jar.disabled')).toBeVisible();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  await page.getByRole('searchbox').clear();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('local-mods-light.png'), fullPage: true });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: testInfo.outputPath('local-mods-dark.png'), fullPage: true });
  await page.getByRole('button', { name: `删除 ${mod.name}` }).click();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(page.getByRole('switch')).toBeVisible();
  await page.getByRole('button', { name: `删除 ${mod.name}` }).click();
  await page.getByRole('button', { name: '确认删除', exact: true }).click();
  await expect(page.getByRole('switch')).toHaveCount(0);
  const calls = await page.evaluate(() => (window as unknown as MockWindow).__calls);
  expect(calls.find((call) => call.command === 'set_game_mod_enabled')?.args).toEqual({ id: game.id, fileName: 'synthetic.jar', enabled: false });
  expect(calls.find((call) => call.command === 'remove_game_mod')?.args).toEqual({ id: game.id, fileName: 'synthetic.jar.disabled' });
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await expect(page.getByRole('article')).toBeVisible();
});

test('local content mutations lock during start and running; Vanilla hint and installed entry only', async ({ page }) => {
  await mock(page, [{ ...game, loader_version: null }, { ...game, id: 'installing-game', name: '安装中', status: 'installing' }]);
  await page.goto('/');
  await expect(page.getByRole('button', { name: '管理内容' })).toHaveCount(1);
  await page.getByRole('button', { name: '管理内容' }).click();
  await expect(page.getByText('原版不会加载 Mod。需要使用 Fabric 等模组加载器。')).toBeVisible();
  await page.getByRole('button', { name: '导入 Mod' }).click();
  await page.getByRole('button', { name: '以 Cobble 开始' }).click();
  await expect(page.getByRole('button', { name: '导入 Mod' })).toBeDisabled();
  await expect(page.getByRole('switch')).toBeDisabled();
  await expect(page.getByRole('button', { name: `删除 ${mod.name}` })).toBeDisabled();
  await page.evaluate((value) => (window as unknown as MockWindow).__emitGame({ ...value, launch_state: 'running' }), game);
  await expect(page.getByRole('button', { name: '运行中', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '导入 Mod' })).toBeDisabled();
  await page.evaluate((value) => (window as unknown as MockWindow).__emitGame({ ...value, launch_state: 'idle' }), game);
  await expect(page.getByRole('button', { name: '导入 Mod' })).toBeEnabled();
});

test('game details rename inline and open only native-owned locations', async ({ page }, testInfo) => {
  await mock(page, [game]);
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
  await expect(page.getByRole('button', { name: '打开日志', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: '更改名称' }).click();
  await expect(page.getByLabel('游戏名称')).toBeFocused();
  await page.getByLabel('游戏名称').fill('取消的名字');
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(page.getByRole('heading', { name: game.name, exact: true })).toBeVisible();
  await page.getByRole('button', { name: '更改名称' }).click();
  await page.getByLabel('游戏名称').fill('  ');
  await page.getByRole('button', { name: '保存名称' }).click();
  await expect(page.getByRole('alert')).toContainText('1–80');
  await page.getByLabel('游戏名称').fill('  新的林间小屋  ');
  await page.evaluate(() => { (window as unknown as MockWindow).__actionFails = true; });
  await page.getByRole('button', { name: '保存名称' }).click();
  await expect(page.getByRole('alert')).toContainText('游戏记录暂时不可用');
  await expect(page.getByLabel('游戏名称')).toHaveValue('  新的林间小屋  ');
  await page.evaluate(() => { (window as unknown as MockWindow).__actionFails = false; });
  await page.getByRole('button', { name: '保存名称' }).click();
  await expect(page.getByRole('heading', { name: '新的林间小屋', exact: true })).toBeVisible();
  await expect(page.getByRole('status')).toHaveText('名称已保存');
  await page.getByRole('button', { name: '打开游戏文件夹', exact: true }).click();
  await page.evaluate((game) => (window as unknown as MockWindow).__emitGame(game), { ...game, name: '新的林间小屋', log_path: 'native-owned-log' } satisfies Game);
  await page.getByRole('button', { name: '打开日志', exact: true }).click();
  const calls = await page.evaluate(() => (window as unknown as MockWindow).__calls);
  expect(calls.filter((call) => call.command === 'rename_game').at(-1)?.args).toEqual({ id: game.id, name: '新的林间小屋' });
  expect(calls.find((call) => call.command === 'open_game_directory')?.args).toEqual({ id: game.id });
  expect(calls.find((call) => call.command === 'open_game_log')?.args).toEqual({ id: game.id });
  await page.evaluate(() => { (window as unknown as MockWindow).__actionFails = true; });
  await page.getByRole('button', { name: '打开游戏文件夹', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('文件已不存在');
  await page.getByRole('button', { name: '更改名称' }).click();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('details-rename-light.png'), fullPage: true });
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await expect(page.getByRole('article', { name: '新的林间小屋' })).toBeVisible();
});

test('pack Mods distinguish included and user-added content without restricting controls', async ({ page }) => {
  await mock(page, [{ ...game, pack_name: '林间整合包', pack_version: '1.0' }], [
    { ...mod, pack_owned: true },
    { ...mod, file_name: 'extra.jar', name: '自行选择的地图', pack_owned: false },
  ]);
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
  const included = page.getByRole('listitem').filter({ hasText: 'synthetic.jar' });
  const added = page.getByRole('listitem').filter({ hasText: 'extra.jar' });
  await expect(included.getByText('整合包自带', { exact: true })).toBeVisible();
  await expect(added.getByText('自行添加', { exact: true })).toBeVisible();
  await expect(included.getByRole('switch')).toBeEnabled();
  await expect(included.getByRole('button', { name: '删除 林间地图', exact: true })).toBeEnabled();
  await expect(added.getByRole('switch')).toBeEnabled();
});

for (const [kind, label, selected] of [['forge', 'Forge', '52.0.28'], ['neoforge', 'NeoForge', '21.1.100']] as const) {
  test(`${label} creation uses the selected kind and recommended or first available version`, async ({ page }, testInfo) => {
    await mock(page);
    await page.goto('/');
    await page.getByRole('button', { name: '创建游戏', exact: true }).click();
    await page.getByRole('radio', { name: label, exact: true }).click();
    await expect(page.getByLabel(`${label} 版本`)).toHaveValue(selected);
    await expect(page.getByText('安装时会运行官方安装器。', { exact: false })).toBeVisible();
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({ path: testInfo.outputPath(`${kind}-create.png`), fullPage: true });
    await page.getByRole('button', { name: '创建并安装' }).click();
    await expect(page.getByRole('article')).toContainText(`${label} ${selected}`);
    await page.evaluate((game) => (window as unknown as MockWindow).__emitGame(game), { ...game, loader_kind: kind, loader_version: selected, status: 'installing', progress: { phase: kind === 'forge' ? 'forge-installer' : 'neoforge-installer', completed: 0, total: 1, reused: 0 } } satisfies Game);
    await expect(page.getByRole('status')).toHaveText(`安装 ${label}`);
    await expect(page.getByRole('progressbar')).toHaveCount(0);
    const args = await page.evaluate(() => (window as unknown as MockWindow).__calls.find((call) => call.command === 'create_game')?.args);
    expect(args?.loaderKind).toBe(kind);
    expect(args?.loaderVersion).toBe(selected);
    const lookup = await page.evaluate(() => (window as unknown as MockWindow).__calls.find((call) => call.command === 'list_loader_versions')?.args);
    expect(lookup).toEqual({ version: '1.21.1', kind, source: { source: 'bmclapi', mirror: '' } });
  });
}

test('unsupported Forge version shows the host reason and cannot submit an empty loader', async ({ page }) => {
  await mock(page);
  await page.goto('/');
  await page.getByRole('button', { name: '创建游戏', exact: true }).click();
  await page.getByRole('radio', { name: 'Forge', exact: true }).click();
  await page.getByLabel('Minecraft 版本').selectOption('1.20.6');
  await expect(page.getByRole('alert')).toContainText('暂不支持此 Minecraft 版本');
  await expect(page.getByLabel('Forge 版本')).toBeDisabled();
  await expect(page.getByRole('button', { name: '创建并安装' })).toBeDisabled();
  await page.getByRole('button', { name: '重试', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('目前支持 1.21.1');
  await page.getByLabel('Minecraft 版本').selectOption('1.21.1');
  await expect(page.getByLabel('Forge 版本')).toHaveValue('52.0.28');
  await expect(page.getByRole('button', { name: '创建并安装' })).toBeEnabled();
});
