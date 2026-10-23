// SPDX-License-Identifier: MIT
// Fixed synthetic desktop data; no public network, real games, or files modified.
import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import type { Game } from '../src/games';
import type { MeshOverview } from '../src/mesh';
const cover = 'data:image/png;base64,' + readFileSync('../../validation/perf-final/original-cover-fixture.png').toString('base64');
for (const state of ['empty','loading','error','longname']) { test('home state ' + state, async ({page},info)=>{ const width=1000;const height=720;const language='en-US';const theme='light'; await page.route('https://**/*',route=>route.abort());
    await page.addInitScript(({ language, theme, state, cover }) => {
      const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __workspaceCalls: string[] };
      target.isTauri = true; target.__workspaceCalls = []; localStorage.setItem('cobble-language', language); localStorage.setItem('cobble.appearance',theme);
      const game: Game = { id: 'navigation-game', name: 'Navigation game', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null, launch_state: 'idle', cover_ref: 'original-fixture', last_played_at: 1 };
      const mesh: MeshOverview = { available: true, reason: null, can_host: true, component_state: 'idle', mode: 'explicit_relay', discovery_state: 'inactive', discovery_source: null, discovery_total: 0, configured_services: [], nodes: [], neighbors: [], capabilities: { relay: true, direct: false, hole_punch: false }, relay: { address: null, source: null, state: 'unconfigured' }, session: null };
      let callback = 0;
      target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      target.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
        invoke: async (command: string) => {
          target.__workspaceCalls.push(command);
          if (command === 'plugin:event|listen') return callback;
          if (command === 'plugin:window|is_maximized') return false;
          if (command === 'list_games') { if (state === 'loading') await new Promise(() => {}); if (state === 'error') throw new Error('synthetic unavailable'); return state === 'empty' ? [] : [{...game,name:state==='longname'?'A very long game name with an extended original fixture description 我的超长游戏名称测试':game.name}, {...game,id:'second-game',name:'Vanilla survival',cover_ref:null}]; } if (command === 'get_game_covers') return [{ id:game.id,cover_ref:'original-fixture',data_url:cover }];
          if (['list_tasks', 'list_plugins', 'list_extension_contributions', 'list_plugin_catalogs', 'list_bundled_plugins', 'list_blueprints', 'list_managed_game_mods', 'list_game_packs', 'list_game_worlds'].includes(command)) return [];
          if (command === 'list_game_mods') return [{ file_name:'fixture.jar',name:'Fixture Mod',version:'1.2.3',authors:['Fixture'],enabled:true,description:'A local synthetic Mod row',loader:'fabric' }]; if (command === 'list_player_profiles') return { profiles: [], active_id: null };
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
    }, {language,theme,state,cover});

await page.setViewportSize({width,height});await page.goto('/');
if(state==='loading') { await expect(page.locator('.home-empty[role="status"]')).toBeVisible(); await expect(page.getByText('No games yet',{exact:true})).toHaveCount(0); }
else if(state==='error') { await expect(page.locator('.home-empty [role="alert"]')).toBeVisible(); await expect(page.getByRole('button',{name:'Retry',exact:true})).toBeVisible(); await expect(page.getByText('No games yet',{exact:true})).toHaveCount(0); }
else if(state==='empty') await expect(page.getByText('No games yet',{exact:true})).toBeVisible();
else await expect(page.locator('.library-game--featured')).toBeVisible();
await page.screenshot({path:info.outputPath(state+'.png'),animations:'disabled'}); }); }

