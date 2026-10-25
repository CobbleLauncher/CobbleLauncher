// SPDX-License-Identifier: MIT
import { expect, test } from '@playwright/test';
import { reverseConfigurationFixture, reverseModFixtures } from '../src/reverse-config/fixtures';

type BoundaryMode = 'normal' | 'empty' | 'search-error' | 'solve-error' | 'create-error' | 'conflict';
type ReverseCall = { command: string; args: Record<string, unknown> };
type ReverseTestWindow = Window & { isTauri: boolean; __reverseMode: BoundaryMode; __reverseCalls: ReverseCall[]; __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => void }; __TAURI_INTERNALS__: { metadata: unknown; transformCallback: () => number; invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> } };
const syntheticMods = reverseModFixtures.map(mod => ({ ...mod, title: `Synthetic ${mod.title}` }));
const syntheticConfiguration = {
  ...reverseConfigurationFixture,
  choices: reverseConfigurationFixture.choices.map(choice => ({ ...choice, plan: { ...choice.plan, modrinth: choice.plan.modrinth ? { ...choice.plan.modrinth, mods: choice.plan.modrinth.mods.map(mod => ({ ...mod, name: `Synthetic ${mod.name}` })) } : null } })),
};

async function openCreate(page: import('@playwright/test').Page, locale: 'zh-CN' | 'en-US', theme: 'light' | 'dark', mode: BoundaryMode = 'normal') {
  await page.addInitScript(({ locale, theme, mode, mods, configuration }) => {
    localStorage.setItem('cobble-language', locale);
    localStorage.setItem('cobble.appearance', theme);
    const w = window as unknown as ReverseTestWindow;
    w.isTauri = true;
    w.__reverseMode = mode;
    w.__reverseCalls = [];
    let callback = 0;
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        w.__reverseCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (['list_games', 'list_tasks', 'list_plugins', 'list_extension_contributions', 'list_plugin_catalogs', 'list_bundled_plugins', 'list_blueprints'].includes(command)) return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'C:/Games', custom_game_directory: false };
        if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
        if (command === 'get_download_tuning') return { max_concurrent: 12, per_origin: 4 };
        if (command === 'get_public_cache_stats') return { disk_bytes: 0 };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
        if (command === 'default_game_directory') return 'C:/Games';
        if (command === 'list_game_versions') return [{ id: '1.21.1', type: 'release', release_time: '2024-06-13T00:00:00Z' }];
        if (command === 'search_environment_mods') {
          if (w.__reverseMode === 'search-error') throw new Error('synthetic search error');
          return { hits: w.__reverseMode === 'empty' ? [] : mods, offset: 0, total_hits: w.__reverseMode === 'empty' ? 0 : mods.length };
        }
        if (command === 'reverse_configure_environment') {
          if (w.__reverseMode === 'solve-error') throw new Error('synthetic solver error');
          if (w.__reverseMode === 'conflict') return { ...configuration, choices: [], conflicts: [{ game_version: '1.21.1', loader: 'fabric', message: 'A selected Mod requires an unavailable dependency.', projects: [{source:'modrinth',project_id:'sodium'},{source:'modrinth',project_id:'iris'}] }] };
          return configuration;
        }
        if (command === 'preview_environment_installation') return { loader_version:'0.16.14',requirements:{ready:true,missing:[]},space:{can_proceed:true,exact:true,volumes:[]} };
        if (command === 'create_game_from_environment') {
          if (w.__reverseMode === 'create-error') throw new Error('synthetic creation error');
          return { id: 'synthetic-created-game', name: args.name, version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.5', directory: 'C:/Games/Synthetic', status: 'installing', error: null, progress: null };
        }
        return null;
      },
    };
  }, { locale, theme, mode, mods: syntheticMods, configuration: syntheticConfiguration });
  await page.goto('/');
  const createButton = page.getByRole('button', { name: locale === 'en-US' ? 'Create game' : '创建游戏', exact: true });
  await createButton.click();
  await expect(page.getByRole('radio', { name: locale === 'en-US' ? 'Start with Mods' : '从 Mod 开始' })).toBeVisible();
  await page.getByRole('radio', { name: locale === 'en-US' ? 'Start with Mods' : '从 Mod 开始' }).click();
}

test('reverse configuration selects Mods, reviews an exact proposal and creates from it', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await openCreate(page, 'zh-CN', 'light');
  await expect(page.getByRole('button', { name: 'Synthetic Sodium', exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Synthetic Sodium', exact: false }).click();
  await page.getByRole('button', { name: 'Synthetic Iris Shaders', exact: false }).click();
  await expect(page.getByRole('button', { name: '查找兼容组合' })).toBeEnabled();
  await page.getByRole('button', { name: '查找兼容组合' }).click();
  await expect(page.getByRole('radio', { name: /Minecraft 1\.21\.1.*Fabric/ })).toBeVisible();
  await page.getByRole('button',{name:'必需依赖（1）',exact:true}).click();
  await expect(page.locator('.reverse-dependencies')).toContainText('Synthetic Fabric API');
  await expect(page.locator('.reverse-dependencies')).toContainText('必需依赖');
  await expect(page.getByLabel('游戏名称')).toBeVisible();
  const calls = await page.evaluate(() => (window as unknown as ReverseTestWindow).__reverseCalls);
  const solve = calls.find(call => call.command === 'reverse_configure_environment');
  expect(solve?.args.roots).toEqual([{source:'modrinth',project_id:'sodium'},{source:'modrinth',project_id:'iris'}]);
  await page.getByRole('button', { name: '创建并安装全部内容' }).click();
  await expect.poll(async () => (await page.evaluate(() => (window as unknown as ReverseTestWindow).__reverseCalls)).some((call) => call.command === 'create_game_from_environment')).toBe(true);
  const create = await page.evaluate(() => (window as unknown as ReverseTestWindow).__reverseCalls.find((call) => call.command === 'create_game_from_environment'));
  expect(create?.args.planId).toBe('reviewed-fabric-plan');
  expect(errors).toEqual([]);
});

test('empty results, search errors, solver errors and creation errors stay recoverable', async ({ page }) => {
  await openCreate(page, 'en-US', 'dark', 'empty');
  await expect(page.getByText('No Mods found. Try another search.', { exact: true })).toBeVisible();
  await page.evaluate(() => { (window as unknown as ReverseTestWindow).__reverseMode = 'search-error'; });
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Could not search this source');

  await page.evaluate(() => { (window as unknown as ReverseTestWindow).__reverseMode = 'conflict'; });
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await page.getByRole('button', { name: 'Synthetic Sodium', exact: false }).click();
  await page.getByRole('button', { name: 'Find combinations', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'No available combination', exact: true })).toBeVisible();
  await page.getByText(/Review unmet requirements/).click();
  await expect(page.locator('.reverse-conflicts')).toContainText('A selected Mod requires an unavailable dependency.');
  await page.getByRole('button', { name: 'Remove Synthetic Sodium' }).click();

  await page.evaluate(() => { (window as unknown as ReverseTestWindow).__reverseMode = 'solve-error'; });
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await page.getByRole('button', { name: 'Synthetic Sodium', exact: false }).click();
  await page.getByRole('button', { name: 'Find combinations', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Could not find combinations');

  await page.evaluate(() => { (window as unknown as ReverseTestWindow).__reverseMode = 'normal'; });
  await page.getByRole('button', { name: 'Find combinations', exact: true }).click();
  await page.getByRole('radio', { name: /Minecraft 1\.21\.1.*Fabric/ }).waitFor();
  await page.evaluate(() => { (window as unknown as ReverseTestWindow).__reverseMode = 'create-error'; });
  await page.getByRole('button', { name: 'Create and install everything', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Could not create the game');
});

test('reverse configuration supports keyboard selection and requested desktop and narrow screenshots', async ({ page }, info) => {
  await openCreate(page, 'zh-CN', 'light');
  await page.setViewportSize({ width: 1280, height: 800 });
  await expect(page.getByRole('button', { name: 'Synthetic Sodium', exact: false })).toBeVisible();
  const remoteRequests: string[] = [];
  page.on('request', request => { if (new URL(request.url()).hostname !== '127.0.0.1') remoteRequests.push(request.url()); });
  await page.getByRole('button', { name: 'Synthetic Sodium', exact: false }).focus();
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Synthetic Iris Shaders', exact: false }).focus();
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: '查找兼容组合' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('radio', { name: /Minecraft 1\.21\.1.*Fabric/ })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('T076-reverse-1280-zh-light.png'), fullPage: true, animations: 'disabled' });

  await openCreate(page, 'en-US', 'dark');
  await page.setViewportSize({ width: 1000, height: 720 });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'Synthetic Sodium', exact: false }).focus();
  await page.keyboard.press('Space');
  await page.getByRole('button', { name: 'Synthetic Iris Shaders', exact: false }).focus();
  await page.keyboard.press('Space');
  await page.getByRole('button', { name: 'Find combinations', exact: true }).focus();
  await page.keyboard.press('Space');
  await expect(page.getByRole('radio', { name: /Minecraft 1\.21\.1.*Fabric/ })).toBeVisible();
  expect(await page.locator('html').evaluate(element => getComputedStyle(element).getPropertyValue('--bg').trim())).toBe('#161a1f');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('button', { name: 'Create and install everything', exact: true }).scrollIntoViewIfNeeded();
  await page.screenshot({ path: info.outputPath('T076-reverse-1000x720-en-dark.png'), animations: 'disabled' });

  await page.setViewportSize({ width: 560, height: 800 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect.poll(() => page.locator('.titlebar').evaluate(element => Math.round(element.getBoundingClientRect().top))).toBeLessThanOrEqual(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('T076-reverse-560x800-en-dark.png'), animations: 'disabled' });
  await page.getByRole('button', { name: 'Create and install everything', exact: true }).scrollIntoViewIfNeeded();
  await page.screenshot({ path: info.outputPath('T076-reverse-560x800-en-dark-create.png'), animations: 'disabled' });
  expect(remoteRequests).toEqual([]);
});
