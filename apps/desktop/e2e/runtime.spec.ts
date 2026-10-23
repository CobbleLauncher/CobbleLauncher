// SPDX-License-Identifier: MIT
// Synthetic native boundary; these tests never start a game process.
import { test, expect, type Page } from '@playwright/test';
import type { Game, LauncherPreferences, ProfileLibrary } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type RuntimeWindow = Window & {
  __calls: Call[];
  __preferences: LauncherPreferences;
  __game: Game;
  __cancelGameJavaPick: boolean;
};
const game: Game = {
  id: 'runtime-test-game', name: '测试世界', version: '1.21.1', directory: 'C:\\Synthetic\\world',
  status: 'installed', error: null, progress: null,
  runtime_overrides: { java: { mode: 'inherit' }, memory_mb: null },
};
const profiles: ProfileLibrary = { profiles: [], active_id: null };

async function mockRuntimeNative(page: Page) {
  await page.addInitScript(({ initialGame, initialProfiles }) => {
    const target = window as unknown as RuntimeWindow & {
      isTauri: boolean;
      __TAURI_INTERNALS__: unknown;
      __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
    };
    target.isTauri = true;
    target.__calls = [];
    target.__preferences = { memory_mb: 2048, game_directory: 'C:\\Cobble\\games', custom_game_directory: false };
    target.__game = structuredClone(initialGame);
    target.__cancelGameJavaPick = true;
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => {
        const id = callbacks.size + 1;
        callbacks.set(id, callback);
        return id;
      },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') return 1;
        if (command === 'list_games') return [target.__game];
        if (command === 'list_player_profiles') return initialProfiles;
        if (command === 'list_game_mods') return [];
        if (command === 'get_download_tuning') return { max_concurrent: 12, per_origin: 4 };
        if (command === 'get_public_cache_stats') return { disk_bytes: 0 };
        if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
        if (command === 'get_download_settings') return { source: 'official', mirror: '' };
        if (command === 'get_launcher_preferences') return structuredClone(target.__preferences);
        if (command === 'set_default_advanced_runtime') {
          target.__preferences = { ...target.__preferences, window_size: args.windowSize as LauncherPreferences['window_size'], jvm_args: args.jvmArgs as string[], launch_behavior: args.launchBehavior as LauncherPreferences['launch_behavior'] };
          return structuredClone(target.__preferences);
        }
        if (command === 'set_game_advanced_runtime') {
          target.__game = { ...target.__game, runtime_overrides: { ...target.__game.runtime_overrides!, window_size: args.windowSize as LauncherPreferences['window_size'], jvm_args: args.jvmArgs as string[] | null, launch_behavior: args.launchBehavior as LauncherPreferences['launch_behavior'] } };
          return structuredClone(target.__game);
        }
        if (command === 'set_default_memory') {
          target.__preferences = { ...target.__preferences, memory_mb: args.memoryMb as number };
          return structuredClone(target.__preferences);
        }
        if (command === 'pick_default_game_directory') {
          target.__preferences = { memory_mb: target.__preferences.memory_mb, game_directory: 'D:\\Custom\\games', custom_game_directory: true };
          return structuredClone(target.__preferences);
        }
        if (command === 'reset_default_game_directory') {
          target.__preferences = { memory_mb: target.__preferences.memory_mb, game_directory: 'C:\\Cobble\\games', custom_game_directory: false };
          return structuredClone(target.__preferences);
        }
        if (command === 'get_java_settings') {
          return { mode: 'custom', executable: 'C:\\GlobalJava\\bin\\java.exe', runtime: { version: '21.0.7', major: 21, arch: 'amd64', vendor: 'Microsoft', executable: 'C:\\GlobalJava\\bin\\java.exe' } };
        }
        if (command === 'set_game_memory') {
          target.__game = { ...target.__game, runtime_overrides: { ...target.__game.runtime_overrides!, memory_mb: args.memoryMb as number | null } };
          return structuredClone(target.__game);
        }
        if (command === 'set_game_java_mode') {
          target.__game = { ...target.__game, runtime_overrides: { ...target.__game.runtime_overrides!, java: { mode: args.mode as 'inherit' | 'automatic' } } };
          return structuredClone(target.__game);
        }
        if (command === 'pick_game_java_executable') {
          if (target.__cancelGameJavaPick) return null;
          target.__game = { ...target.__game, runtime_overrides: { ...target.__game.runtime_overrides!, java: { mode: 'custom', executable: 'D:\\GameJava\\bin\\java.exe' } } };
          return structuredClone(target.__game);
        }
        if (command === 'plugin:window|is_maximized') return false;
        return null;
      },
    };
  }, { initialGame: game, initialProfiles: profiles });
}

async function openGameRuntime(page: Page) {
  await page.getByRole('button', { name: '管理内容', exact: true }).click();
  await page.getByRole('tab', { name: '运行设置', exact: true }).click();
}

test('global memory saves and changing then resetting the default directory leaves existing games in place', async ({ page }, testInfo) => {
  await mockRuntimeNative(page);
  await page.goto('/');
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('tab', { name: '默认运行设置', exact: true }).click();
  await page.getByRole('group', { name: '默认最大内存 (MiB)预设' }).getByRole('button', { name: '4 GiB' }).click();
  await page.getByRole('button', { name: '保存默认内存' }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as RuntimeWindow).__preferences.memory_mb)).toBe(4096);
  expect(await page.evaluate(() => (window as unknown as RuntimeWindow).__calls.find((call) => call.command === 'set_default_memory')?.args)).toEqual({ memoryMb: 4096 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('runtime-settings-light.png'), fullPage: true, animations: 'disabled' });

  await page.getByRole('tab', { name: '外观', exact: true }).click();
  await page.getByRole('radio', { name: '深色', exact: true }).click();
  await page.getByRole('tab', { name: '默认运行设置', exact: true }).click();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('runtime-settings-dark.png'), fullPage: true, animations: 'disabled' });
  await page.setViewportSize({ width: 560, height: 420 });
  await page.getByRole('button', { name: '保存默认内存' }).scrollIntoViewIfNeeded();
  await expect(page.getByRole('button', { name: '保存默认内存' })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);

  await page.getByRole('tab', { name: '下载与目录', exact: true }).click();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('runtime-settings-minimum-window.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '更改目录', exact: true }).click();
  await expect(page.getByText('D:\\Custom\\games', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '恢复默认目录', exact: true }).click();
  await expect(page.getByText('C:\\Cobble\\games', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '返回游戏库' }).click();
  await expect(page.getByRole('article', { name: '测试世界' }).getByText(game.directory, { exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as RuntimeWindow).__game.directory)).toBe(game.directory);
});

test('per-game memory override can be restored to inherit', async ({ page }, testInfo) => {
  await mockRuntimeNative(page);
  await page.goto('/');
  await openGameRuntime(page);
  await expect(page.getByText('跟随默认 · 2048 MiB', { exact: true })).toBeVisible();
  await page.getByLabel('内存设置').selectOption('custom');
  await page.getByRole('group', { name: '游戏最大内存 (MiB)预设' }).getByRole('button', { name: '4 GiB' }).click();
  await page.getByRole('button', { name: '保存游戏内存' }).click();
  await expect(page.getByText('此游戏使用 4096 MiB', { exact: true })).toBeVisible();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('per-game-runtime-memory-light.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '恢复默认内存' }).click();
  await expect(page.getByText('跟随默认 · 2048 MiB', { exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as RuntimeWindow).__calls.filter((call) => call.command === 'set_game_memory').map((call) => call.args))).toEqual([
    { id: game.id, memoryMb: 4096 },
    { id: game.id, memoryMb: null },
  ]);
});

test('game automatic Java ignores a custom global Java and picker cancellation preserves the selected mode', async ({ page }) => {
  await mockRuntimeNative(page);
  await page.goto('/');
  await openGameRuntime(page);
  await expect(page.getByText('跟随默认 · 指定 Java', { exact: true })).toBeVisible();
  await page.getByLabel('Java模式').selectOption('automatic');
  await expect(page.getByText('按游戏版本自动选择 Java', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '选择游戏 Java', exact: true }).click();
  await expect(page.getByLabel('Java模式')).toHaveValue('automatic');
  expect(await page.evaluate(() => (window as unknown as RuntimeWindow).__calls.filter((call) => call.command === 'set_game_java_mode').map((call) => call.args))).toEqual([{ id: game.id, mode: 'automatic' }]);
  expect(await page.evaluate(() => (window as unknown as RuntimeWindow).__calls.filter((call) => call.command === 'pick_game_java_executable').map((call) => call.args))).toEqual([{ id: game.id }]);
});


test('advanced defaults and game overrides save direct JVM argv and restore inheritance', async ({ page }) => {
  await mockRuntimeNative(page);
  await page.goto('/');
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('tab', { name: '默认运行设置', exact: true }).click();
  await page.locator('.advanced-runtime-settings > summary').click();
  await page.getByLabel('窗口宽度', { exact: true }).fill('1280');
  await page.getByLabel('窗口高度', { exact: true }).fill('720');
  await page.getByLabel('额外 JVM 参数', { exact: true }).fill('-Dlabel=two words\n-XX:+UseG1GC');
  await page.getByLabel('游戏启动后', { exact: true }).selectOption('minimize');
  await page.getByRole('button', { name: '保存启动设置', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('启动设置已保存');
  const defaults = await page.evaluate(() => (window as unknown as RuntimeWindow).__calls.find(call => call.command === 'set_default_advanced_runtime'));
  expect(defaults?.args).toEqual({ windowSize: { width: 1280, height: 720 }, jvmArgs: ['-Dlabel=two words', '-XX:+UseG1GC'], launchBehavior: 'minimize' });
  await page.getByRole('button', { name: '返回游戏库', exact: true }).click();
  await openGameRuntime(page);
  await page.locator('.advanced-runtime-settings > summary').click();
  await page.getByLabel('游戏窗口', { exact: true }).selectOption('false');
  await page.getByLabel('窗口宽度', { exact: true }).fill('854');
  await page.getByLabel('窗口高度', { exact: true }).fill('480');
  await page.getByLabel('额外 JVM 参数', { exact: true }).selectOption('false');
  await page.getByLabel('JVM 参数', { exact: true }).fill('-Dgame=forest');
  await page.getByLabel('游戏启动后', { exact: true }).selectOption('keep');
  await page.getByRole('button', { name: '保存启动设置', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as RuntimeWindow).__game.runtime_overrides?.window_size?.width)).toBe(854);
  await page.getByLabel('游戏窗口', { exact: true }).selectOption('true');
  await page.getByLabel('额外 JVM 参数', { exact: true }).selectOption('true');
  await page.getByLabel('游戏启动后', { exact: true }).selectOption('inherit');
  await page.getByRole('button', { name: '保存启动设置', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as RuntimeWindow).__game.runtime_overrides?.window_size)).toBeNull();
  const restored = await page.evaluate(() => (window as unknown as RuntimeWindow).__calls.filter(call => call.command === 'set_game_advanced_runtime').at(-1));
  expect(restored?.args).toEqual({ id: game.id, windowSize: null, jvmArgs: null, launchBehavior: null });
});
