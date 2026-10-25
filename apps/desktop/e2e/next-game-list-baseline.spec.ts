// SPDX-License-Identifier: MIT
// Synthetic browser fixture only; its timings are not native launcher benchmarks.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import type { Game, LocalMod } from '../src/games';

async function scrollFrames(page: Page, rounds = 3) {
  const samples: number[][] = [];
  for (let round = 0; round < rounds; round++) {
    samples.push(await page.evaluate(async () => {
      const root = document.scrollingElement!;
      const max = Math.max(0, root.scrollHeight - innerHeight);
      const durations: number[] = [];
      let previous = await new Promise<number>(resolve => requestAnimationFrame(resolve));
      for (let index = 0; index < 40; index++) {
        root.scrollTop = (max * ((index % 20) / 19));
        await new Promise<void>(resolve => requestAnimationFrame(now => { durations.push(now - previous); previous = now; resolve(); }));
      }
      root.scrollTop = 0;
      return durations;
    }));
  }
  const values = samples.flat().sort((a, b) => a - b);
  return { rounds, framesPerRound: 40, p50Ms: values[Math.floor(values.length * .5)], p95Ms: values[Math.floor(values.length * .95)], maxMs: values.at(-1), samples };
}

for (const gameCount of [100, 1000]) { const width = 1100;
  test(`synthetic ${gameCount}-game browser baseline at ${width}px`, async ({ page }) => {
    const remoteRequests: string[] = [];
    await page.route('https://**/*', route => { remoteRequests.push(route.request().url()); return route.abort(); });
    await page.addInitScript(({gameCount}) => {
      const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
      target.isTauri = true;
      localStorage.setItem('cobble-language', 'en-US');
      const games: Game[] = Array.from({ length: gameCount }, (_, index) => ({
        id: `fixture-${String(index).padStart(3, '0')}`, name: `Fixture game ${String(index).padStart(3, '0')}`, version: '1.21.1',
        loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null,
        last_launched_at: index === 0 ? 1 : 0,
      }));
      const mods: LocalMod[] = Array.from({ length: 1000 }, (_, index) => ({
        file_name: `fixture-mod-${String(index).padStart(4, '0')}.jar`, name: `Fixture Mod ${String(index).padStart(4, '0')}`,
        version: '1.0.0', authors: ['Fixture'], loader: 'Fabric', enabled: true, description: null,
      }));
      let callback = 0;
      target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      target.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
        invoke: async (command: string) => {
          if (command === 'plugin:event|listen') return callback;
          if (command === 'plugin:window|is_maximized') return false;
          if (command === 'list_games') return games;
          if (command === 'list_game_mods') return mods;
          if (command === 'list_player_profiles') return { profiles: [], active_id: null };
          if (command === 'list_tasks' || command === 'list_plugins' || command === 'list_extension_contributions' || command === 'list_plugin_catalogs' || command === 'list_bundled_plugins') return [];
          return null;
        },
      };
    }, {gameCount});

    await page.setViewportSize({ width, height: 800 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const homeUsableSamplesMs: number[] = [];
    for (let sample = 0; sample < 3; sample++) {
      const homeStarted = Date.now();
      if (sample === 0) await page.goto('/'); else await page.reload();
      await expect(page.locator('.app-workspace h1:visible')).toHaveCount(1);
      await expect(page.locator('.game-card')).toHaveCount(5);
      homeUsableSamplesMs.push(Date.now() - homeStarted);
    }

    await page.getByRole('button', { name: 'All games', exact: true }).click();
    await expect(page.locator('.home-search [role="status"]')).toHaveText(String(gameCount));
    await expect(page.locator('.library-game--row')).toHaveCount(50);
    const pageNextStart = await page.evaluate(() => performance.now());
    await page.getByRole('button', { name: 'Next', exact: true }).click();
    await expect(page.locator('.library-game--row')).toHaveCount(50);
    const pageNextMs = await page.evaluate(start => performance.now() - start, pageNextStart);
    const gameSearchSamplesMs: number[] = [];
    for (const query of ['Fixture game 012', 'Fixture game 044', 'Fixture game 099']) {
      const gameSearchStart = await page.evaluate(() => performance.now());
      await page.getByRole('searchbox', { name: 'Search games' }).fill(query);
      await expect(page.locator('.library-game--row')).toHaveCount(1);
      gameSearchSamplesMs.push(await page.evaluate(start => performance.now() - start, gameSearchStart));
      await page.getByRole('searchbox', { name: 'Search games' }).fill('');
      await expect(page.locator('.library-game--row')).toHaveCount(50);
    }
    await expect(page.locator('.library-game--row')).toHaveCount(50);
    const homeScrollFrames = await scrollFrames(page);

    const report = {
      scope: 'Synthetic browser fixture only; first fresh navigation then two same-context reloads; not a native startup, memory, or performance-P95 claim.',
      recordedAt: new Date().toISOString(),
      browser: await page.evaluate(() => navigator.userAgent),
      viewport: { width, height: 800 },
      fixture: { registeredGames: gameCount, renderedGamesPerPage: 50 },
      measurements: { homeUsable: {samplesMs:homeUsableSamplesMs}, pageNextMs, gameSearch: {samplesMs:gameSearchSamplesMs}, homeScrollFrames },
      remoteRequests,
    };
    const reportPath = resolve(process.env.COBBLE_PERF_REPORT_DIR ?? resolve(import.meta.dirname, '../../../target/perf-next-baseline'), `games-${gameCount}.json`);
    mkdirSync(dirname(reportPath), { recursive: true });
    writeFileSync(reportPath, JSON.stringify(report, null, 2));
    console.log('GAME_LIST_FIXTURE', JSON.stringify({ games: gameCount, homeMs: homeUsableSamplesMs, pageNextMs, searchMs: gameSearchSamplesMs, frameP95Ms: homeScrollFrames.p95Ms }));

    expect(remoteRequests).toEqual([]);
  });
}



