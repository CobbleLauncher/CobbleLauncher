// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only: no game, Mesh process, provider request, or preference mutation is started.
import { expect, test, type Page } from '@playwright/test';
import { writeFileSync } from 'node:fs';
import type { Game } from '../src/games';
import type { MeshOverview } from '../src/mesh';

async function checkWorkspace(page: Page, content: string) {
  await expect(page.locator(content)).toBeVisible();
  await expect(page.locator('.app-workspace h1:visible')).toHaveCount(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const edges = await page.evaluate(selector => {
    const header = document.querySelector('.page-header')!.getBoundingClientRect();
    const body = document.querySelector(selector)!.getBoundingClientRect();
    return { header: header.left, body: body.left };
  }, content);
  expect(Math.abs(edges.header - edges.body)).toBeLessThanOrEqual(1);
}

for (const width of [1100]) {
  test(`browser performance fixture at ${width}px`, async ({ page }, info) => {
    const remoteRequests: string[] = []; await page.route('https://**/*', route => { remoteRequests.push(route.request().url()); return route.abort(); }); const started = Date.now(); const timings: Record<string, number> = {}; const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __workspaceCalls: string[] };
      target.isTauri = true; target.__workspaceCalls = []; localStorage.setItem('cobble-language', 'en-US');
      const game: Game = { id: 'navigation-game', name: 'Navigation game', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null, launch_state: 'running' };
      const mesh: MeshOverview = { available: true, reason: null, can_host: true, component_state: 'idle', mode: 'explicit_relay', discovery_state: 'inactive', discovery_source: null, discovery_total: 0, configured_services: [], nodes: [], neighbors: [], capabilities: { relay: true, direct: false, hole_punch: false }, relay: { address: null, source: null, state: 'unconfigured' }, session: null };
      let callback = 0;
      target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      target.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
        invoke: async (command: string) => {
          target.__workspaceCalls.push(command);
          if (command === 'plugin:event|listen') return callback;
          if (command === 'plugin:window|is_maximized') return false;
          if (command === 'list_games') return [game];
          if (['list_tasks', 'list_plugins', 'list_extension_contributions', 'list_plugin_catalogs', 'list_bundled_plugins', 'list_blueprints', 'list_game_mods', 'list_managed_game_mods', 'list_game_packs', 'list_game_worlds'].includes(command)) return [];
          if (command === 'list_player_profiles') return { profiles: [], active_id: null };
          if (command === 'mesh_overview') return mesh;
          if (command === 'mesh_host_options') return { game_id: game.id, lan: { state: 'not_open', port: null }, relay: mesh.relay };
          if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'C:/Synthetic games', custom_game_directory: false };
          if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
          if (command === 'get_download_tuning') return { max_concurrent: 12, per_origin: 4 };
          if (command === 'get_public_cache_stats') return { disk_bytes: 0 };
          if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
          if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
          if (command === 'search_modrinth_packs') return { hits: Array.from({ length: 40 }, (_, index) => ({ project_id: 'fixture-' + index, slug: 'fixture-' + index, title: 'Fixture pack ' + index, description: 'Fixed local performance fixture', author: 'Fixture', downloads: 42000, icon_url: 'https://cdn.modrinth.com/fixture/shared.png' })), offset: 0, total_hits: 40 }; if (command === 'get_market_image') return 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9ZQmcAAAAASUVORK5CYII=';
          if (command === 'get_curseforge_availability') return { available: false, reason: null };
          return null;
        },
      };
    });
    await page.setViewportSize({ width, height: 850 }); await page.emulateMedia({ reducedMotion: 'reduce' }); await page.goto('/');
    await expect(page.locator('.app-workspace h1:visible')).toHaveCount(1);
    timings.homeReadyMs = Date.now() - started; writeFileSync(info.outputPath('initial-resources.json'), JSON.stringify(await page.evaluate(() => performance.getEntriesByType('resource').filter(item => item.name.includes('.js')).map(item => { const resource = item as PerformanceResourceTiming; return { name:resource.name,transferSize:resource.transferSize,decodedBodySize:resource.decodedBodySize }; })), null, 2)); await page.screenshot({ path: info.outputPath('home-current.png'), animations: 'disabled' }); await page.getByRole('button', { name: 'Manage Navigation game', exact: true }).click();
    await checkWorkspace(page, '.details-page');
    await page.getByRole('button', { name: 'Mesh multiplayer', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Mesh multiplayer', level: 1, exact: true })).toBeVisible();
    await checkWorkspace(page, '.mesh-page');
    await expect(page.locator('.back-button:visible')).toHaveCount(1);
    await page.screenshot({ path: info.outputPath(`mesh-${width}.png`), animations: 'disabled' });
    await page.getByRole('button', { name: 'Back to Navigation game', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Navigation game', level: 1, exact: true })).toBeVisible();
    await checkWorkspace(page, '.details-page');
    await page.getByRole('button', { name: 'Tools', exact: true }).click();
    await checkWorkspace(page, '.tool-catalog');
    await page.screenshot({ path: info.outputPath(`tools-${width}.png`), animations: 'disabled' });
    await expect(page.locator('.back-button:visible')).toHaveCount(0);
    await page.getByRole('button', { name: /^Blueprint library/ }).click();
    await expect(page.getByRole('heading', { name: 'Blueprint library', level: 1, exact: true })).toBeVisible();
    await checkWorkspace(page, '.tools-page');
    await expect(page.locator('.back-button:visible')).toHaveCount(1);
    await expect(page.getByRole('button', { name: /^(Back to Tools|All tools)$/ })).toHaveCount(1);
    await page.screenshot({ path: info.outputPath(`blueprints-${width}.png`), animations: 'disabled' });
    await page.getByRole('button', { name: 'Back to Tools', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Tools', level: 1, exact: true })).toBeVisible();
    for (const [name, selector] of [['Market', '.market-page'], ['Settings', '.settings-layout']] as const) {
      const switchStarted = Date.now(); await page.getByRole('button', { name, exact: true }).click();
      await expect(page.getByRole('heading', { name, level: 1, exact: true })).toBeVisible();
      await expect(page.locator('.back-button:visible')).toHaveCount(0);
      await checkWorkspace(page, selector); timings[name + 'SwitchMs'] = Date.now() - switchStarted; if (name === 'Market') { await expect(page.locator('.pack-result-card')).toHaveCount(40); await expect(page.locator('.market-image img').first()).toBeVisible(); timings.marketListImagesReadyMs = Date.now() - switchStarted; const imageCalls = await page.evaluate(() => (window as unknown as { __workspaceCalls: string[] }).__workspaceCalls.filter(command => command === 'get_market_image').length); expect(imageCalls).toBe(1); timings.imageInvokeCount = imageCalls; }
      await page.screenshot({ path: info.outputPath(`${name.toLowerCase()}-${width}.png`), animations: 'disabled' });
    }
    writeFileSync(info.outputPath('browser-timings.json'), JSON.stringify(timings, null, 2)); await info.attach('browser-measures', { body: JSON.stringify({ timings, browser: await page.evaluate(() => navigator.userAgent), resources: await page.evaluate(() => performance.getEntriesByType('resource').map(item => ({ name: item.name, duration: item.duration }))) }, null, 2), contentType: 'application/json' }); console.log('BROWSER_PERF', JSON.stringify(timings)); expect(errors).toEqual([]); expect(remoteRequests).toEqual([]);
    expect(await page.evaluate(() => (window as unknown as { __workspaceCalls: string[] }).__workspaceCalls.some(command => ['mesh_host', 'mesh_join', 'launch_game', 'save_launcher_preferences'].includes(command)))).toBe(false);
  });
}




