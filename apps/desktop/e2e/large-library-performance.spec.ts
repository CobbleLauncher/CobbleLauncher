// SPDX-License-Identifier: MIT
// Production UI with synthetic in-process IPC data. No native launcher timing claim.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import type { Game, LocalMod } from '../src/games';
import type { ExtensionContribution } from '../src/extensions/contributions';

type InputStep = { query: string; expectedCount: number; expectedText?: string };
type FixtureWindow = Window & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __largeLibraryFixture: { games: number; mods: number; commands: number; calls: string[] } };

function frameSummary(samples: number[][]) {
  const values = samples.flat().toSorted((a, b) => a - b);
  const percentile = (value: number) => values[Math.max(0, Math.ceil(values.length * value) - 1)] ?? null;
  return { sampleCount: values.length, p50Ms: percentile(.5), p95Ms: percentile(.95), maxMs: values.at(-1) ?? null, samples };
}
async function nextFrame(page: Page) { await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => resolve()))); }
async function scrollFrames(page: Page, selector: string) {
  const rounds = [];
  for (let round = 0; round < 3; round++) rounds.push(await page.evaluate(async selector => {
    const anchor = document.querySelector(selector);
    if (!anchor) throw Error(`Missing scroll anchor: ${selector}`);
    let root = document.scrollingElement as HTMLElement;
    for (let parent = anchor.parentElement; parent && parent !== document.body; parent = parent.parentElement) {
      if (/(auto|scroll)/.test(getComputedStyle(parent).overflowY) && parent.scrollHeight > parent.clientHeight + 1) { root = parent; break; }
    }
    const maxScrollPx = Math.max(0, root.scrollHeight - root.clientHeight);
    const durations: number[] = [];
    let previous = await new Promise<number>(resolve => requestAnimationFrame(resolve));
    for (let index = 0; index < 40; index++) {
      root.scrollTop = maxScrollPx * ((index % 20) / 19);
      await new Promise<void>(resolve => requestAnimationFrame(now => { durations.push(now - previous); previous = now; resolve(); }));
    }
    root.scrollTop = 0;
    return { durations, maxScrollPx, container: root === document.scrollingElement ? 'document' : `${root.tagName}.${root.className}` };
  }, selector));
  expect(rounds.every(round => round.maxScrollPx > 0)).toBe(true);
  return { rounds: rounds.length, framesPerRound: 40, container: rounds[0]!.container, maxScrollPx: rounds[0]!.maxScrollPx, ...frameSummary(rounds.map(round => round.durations)) };
}
async function inputFrames(page: Page, inputSelector: string, rowSelector: string, steps: InputStep[]) {
  const result = await page.evaluate(async ({ inputSelector, rowSelector, steps }) => {
    const input = document.querySelector<HTMLInputElement>(inputSelector);
    if (!input) throw Error(`Missing input: ${inputSelector}`);
    input.focus();
    const samples: number[][] = [], observations: { query: string; firstMatchingFrameMs: number | null; renderedRows: number }[] = [];
    let previous = await new Promise<number>(resolve => requestAnimationFrame(resolve));
    for (const step of steps) {
      const started = performance.now();
      input.value = step.query;
      input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: step.query }));
      let firstMatchingFrameMs: number | null = null;
      const durations: number[] = [];
      for (let frame = 0; frame < 8; frame++) {
        await new Promise<void>(resolve => requestAnimationFrame(now => { durations.push(now - previous); previous = now; resolve(); }));
        const rows = document.querySelectorAll(rowSelector);
        if (firstMatchingFrameMs === null && rows.length === step.expectedCount && (!step.expectedText || rows[0]?.textContent?.includes(step.expectedText))) firstMatchingFrameMs = performance.now() - started;
      }
      const renderedRows = document.querySelectorAll(rowSelector).length;
      if (firstMatchingFrameMs === null) throw Error(`Input did not produce expected rows: ${step.query}`);
      observations.push({ query: step.query, firstMatchingFrameMs, renderedRows }); samples.push(durations);
    }
    return { observations, samples };
  }, { inputSelector, rowSelector, steps });
  return { method: 'DOM InputEvent to first matching animation frame; local client filtering only', framesPerStep: 8, observations: result.observations, ...frameSummary(result.samples) };
}

for (const width of [1100, 560]) test(`production synthetic 100-game/1000-mod/1000-command fixture at ${width}px`, async ({ page }, info) => {
  const remoteRequests: string[] = [], pageErrors: string[] = [];
  const allowedOrigin = new URL(String(info.project.use.baseURL)).origin;
  page.on('pageerror', error => pageErrors.push(error.message));
  await page.route('**/*', route => {
    const url = new URL(route.request().url());
    if (url.origin === allowedOrigin) return route.continue();
    remoteRequests.push(url.href); return route.abort();
  });
  await page.addInitScript(() => {
    const target = window as unknown as FixtureWindow;
    target.isTauri = true;
    localStorage.clear(); localStorage.setItem('cobble-language', 'en-US'); localStorage.setItem('cobble.appearance', 'light');
    const games: Game[] = Array.from({ length: 100 }, (_, index) => ({ id: `fixture-${String(index).padStart(3, '0')}`, name: `Fixture game ${String(index).padStart(3, '0')}`, version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null, last_launched_at: index === 0 ? 1 : 0 }));
    const mods: LocalMod[] = Array.from({ length: 1000 }, (_, index) => ({ file_name: `fixture-mod-${String(index).padStart(4, '0')}.jar`, name: `Fixture Mod ${String(index).padStart(4, '0')}`, version: '1.0.0', authors: ['Fixture'], loader: 'Fabric', enabled: true, description: null }));
    const contributions: ExtensionContribution[] = Array.from({ length: 25 }, (_, group) => ({
      pluginId: `org.cobble.fixture${group}`, pluginName: `Fixture group ${group}`, settings: [],
      commands: Array.from({ length: 40 }, (_, item) => { const number = String(group * 40 + item).padStart(4, '0'); return { id: `fixture.action.${number}`, label: `Fixture action ${number}`, workflow: 'report', menus: [{ position: 'tools', when: 'always' }, { position: 'instance', when: 'instance' }], searchKeywords: ['fixture local command'] }; }),
    }));
    target.__largeLibraryFixture = { games: games.length, mods: mods.length, commands: contributions.reduce((sum, group) => sum + group.commands.length, 0), calls: [] };
    let callback = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
      invoke: async (command: string) => {
        target.__largeLibraryFixture.calls.push(command);
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return games;
        if (command === 'list_game_mods') return mods;
        if (command === 'list_extension_contributions') return contributions;
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (['list_tasks', 'list_plugins', 'list_plugin_catalogs', 'list_bundled_plugins', 'list_managed_game_mods', 'list_game_curseforge_mods', 'list_game_packs', 'list_game_worlds', 'get_game_covers', 'get_game_icons'].includes(command)) return [];
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'C:/Synthetic games', custom_game_directory: false };
        if (command === 'launcher_update_status') return { version: 'fixture', background_check: false, channel: 'stable', configured: false, can_install: false, available_version: null };
        if (command === 'mesh_overview') return { available: false, reason: 'Synthetic offline fixture', can_host: false, component_state: 'idle', mode: 'off', discovery_state: 'inactive', discovery_source: null, discovery_total: 0, configured_services: [], nodes: [], neighbors: [], capabilities: { relay: false, direct: false, hole_punch: false }, relay: { address: null, source: null, state: 'unconfigured' }, session: null };
        return null;
      },
    };
  });
  await page.setViewportSize({ width, height: 800 }); await page.emulateMedia({ reducedMotion: 'reduce' });
  if (process.env.COBBLE_PERF_INPUT_ONLY === '1') {
    await page.goto('/'); await expect(page.locator('.library-game')).toHaveCount(5); await nextFrame(page);
    await expect(page.locator('script[src*="/@vite/client"]')).toHaveCount(0);
    const productionScripts = await page.evaluate(() => [...document.scripts].map(script => script.src).filter(Boolean));
    await page.locator('.home-more-games').click(); await expect(page.locator('.library-game--row')).toHaveCount(50);
    await page.locator('.home-pagination').getByRole('button', { name: 'Next', exact: true }).click();
    await expect(page.locator('.home-game-list .library-game-open').first()).toHaveText('Fixture game 050'); await nextFrame(page);
    const gameInputs = await inputFrames(page, '.home-search input', '.library-game--row', ['012', '044', '099'].flatMap(id => [{ query: `Fixture game ${id}`, expectedCount: 1, expectedText: `Fixture game ${id}` }, { query: '', expectedCount: 50 }]));
    await page.locator('.home-game-list .library-game-open').filter({ hasText: 'Fixture game 000' }).click();
    await expect(page.locator('.game-mods-panel .mod-row')).toHaveCount(50);
    await page.locator('.game-mods-panel .plugin-list-pagination').getByRole('button', { name: 'Next', exact: true }).click();
    await expect(page.locator('.game-mods-panel .mod-row').first()).toContainText('Fixture Mod 0050'); await nextFrame(page);
    const modInputs = await inputFrames(page, '.game-mods-panel input[type=search]', '.game-mods-panel .mod-row', ['0001', '0399', '0999'].flatMap(id => [{ query: `Fixture Mod ${id}`, expectedCount: 1, expectedText: `Fixture Mod ${id}` }, { query: '', expectedCount: 50 }]));
    const fixture = await page.evaluate(() => (window as unknown as FixtureWindow).__largeLibraryFixture);
    const report = {
      scope: 'Affected input sections only; same synthetic data, queries, paging preconditions and frame loop as the original production baseline. No native performance claim.',
      frameSampleNote: 'P50/P95 describe only captured requestAnimationFrame intervals during the six input steps per list.',
      recordedAt: new Date().toISOString(), browser: await page.evaluate(() => navigator.userAgent), previewOrigin: allowedOrigin, productionScripts, viewport: { width, height: 800 },
      fixture: { registeredGames: fixture.games, renderedGamesPerPage: 50, installedMods: fixture.mods, renderedModsPerPage: 50 }, measurements: { gameInputs, modInputs }, remoteRequests, pageErrors,
    };
    const reportPath = resolve(import.meta.dirname, '../../../validation/t077-production-performance', `large-library-input-${width}.json`);
    mkdirSync(dirname(reportPath), { recursive: true }); writeFileSync(reportPath, JSON.stringify(report, null, 2));
    await info.attach(`large-library-input-${width}`, { path: reportPath, contentType: 'application/json' });
    console.log('LARGE_LIBRARY_INPUT_FIXTURE', JSON.stringify({ width, reportPath }));
    expect(fixture.games).toBe(100); expect(fixture.mods).toBe(1000); expect(remoteRequests).toEqual([]); expect(pageErrors).toEqual([]);
    return;
  }
  const homeNavigationSamplesMs: number[] = [];
  for (let sample = 0; sample < 3; sample++) {
    const started = Date.now(); if (sample === 0) await page.goto('/'); else await page.reload();
    await expect(page.locator('.library-game')).toHaveCount(5); await nextFrame(page); homeNavigationSamplesMs.push(Date.now() - started);
  }
  await expect(page.locator('script[src*="/@vite/client"]')).toHaveCount(0);
  const productionScripts = await page.evaluate(() => [...document.scripts].map(script => script.src).filter(Boolean));
  await page.locator('.home-more-games').click();
  await expect(page.locator('.home-search [role="status"]')).toHaveText('100'); await expect(page.locator('.library-game--row')).toHaveCount(50);
  const gamePageStarted = await page.evaluate(() => performance.now()); await page.locator('.home-pagination').getByRole('button', { name: 'Next', exact: true }).click();
  await expect(page.locator('.home-game-list .library-game-open').first()).toHaveText('Fixture game 050'); await nextFrame(page);
  const gamePageNextMs = await page.evaluate(start => performance.now() - start, gamePageStarted);
  const gameInputs = await inputFrames(page, '.home-search input', '.library-game--row', ['012', '044', '099'].flatMap(id => [{ query: `Fixture game ${id}`, expectedCount: 1, expectedText: `Fixture game ${id}` }, { query: '', expectedCount: 50 }]));
  const homeScroll = await scrollFrames(page, '.home-game-list');
  await page.locator('.home-game-list .library-game-open').filter({ hasText: 'Fixture game 000' }).click();
  await expect(page.getByRole('heading', { name: 'Fixture game 000', exact: true })).toBeVisible(); await expect(page.locator('.game-mods-panel .mod-row')).toHaveCount(50);
  const modPageStarted = await page.evaluate(() => performance.now()); await page.locator('.game-mods-panel .plugin-list-pagination').getByRole('button', { name: 'Next', exact: true }).click();
  await expect(page.locator('.game-mods-panel .mod-row').first()).toContainText('Fixture Mod 0050'); await nextFrame(page);
  const modPageNextMs = await page.evaluate(start => performance.now() - start, modPageStarted);
  const modInputs = await inputFrames(page, '.game-mods-panel input[type=search]', '.game-mods-panel .mod-row', ['0001', '0399', '0999'].flatMap(id => [{ query: `Fixture Mod ${id}`, expectedCount: 1, expectedText: `Fixture Mod ${id}` }, { query: '', expectedCount: 50 }]));
  const modScroll = await scrollFrames(page, '.game-mods-panel');
  const paletteStarted = await page.evaluate(() => performance.now()); await page.keyboard.press('Control+k');
  await expect(page.getByRole('dialog')).toBeVisible(); await page.getByRole('searchbox', { name: 'Search actions', exact: true }).fill('Fixture action');
  await expect(page.locator('#command-results [role=option]')).toHaveCount(50); await nextFrame(page);
  const paletteOpenAndLoadMs = await page.evaluate(start => performance.now() - start, paletteStarted);
  const commandInputs = await inputFrames(page, '.command-dialog input[type=search]', '#command-results [role=option]', ['0001', '0399', '0999'].flatMap(id => [{ query: `Fixture action ${id}`, expectedCount: 1, expectedText: `Fixture action ${id}` }, { query: 'Fixture action', expectedCount: 50 }]));
  await page.keyboard.press('Escape'); await expect(page.getByRole('dialog')).toHaveCount(0);
  const fixture = await page.evaluate(() => (window as unknown as FixtureWindow).__largeLibraryFixture);
  const report = {
    scope: 'Synthetic data transport into production browser UI. Not a native startup, process-memory, download, game-launch, or OS cold-start benchmark.',
    navigationSampleNote: 'One fresh navigation and two same-context reload observations; three raw samples do not establish startup P95.',
    frameSampleNote: 'P50/P95 describe only captured requestAnimationFrame intervals during these synthetic input/scroll loops.',
    recordedAt: new Date().toISOString(), browser: await page.evaluate(() => navigator.userAgent), previewOrigin: allowedOrigin, productionScripts, viewport: { width, height: 800 },
    fixture: { registeredGames: fixture.games, renderedGamesPerPage: 50, installedMods: fixture.mods, renderedModsPerPage: 50, syntheticExtensionCommandCandidates: fixture.commands, paletteInitialRenderedOptions: 50 },
    measurements: { homeNavigationSamplesMs, gamePageNextMs, modPageNextMs, gameInputs, homeScroll, modInputs, modScroll, paletteOpenAndLoadMs, commandInputs },
    remoteRequests, pageErrors,
  };
  const reportPath = resolve(import.meta.dirname, '../../../validation/t077-production-performance', `large-library-${width}.json`); mkdirSync(dirname(reportPath), { recursive: true }); writeFileSync(reportPath, JSON.stringify(report, null, 2));
  await info.attach(`large-library-${width}`, { path: reportPath, contentType: 'application/json' });
  console.log('LARGE_LIBRARY_FIXTURE', JSON.stringify({ width, homeNavigationSamplesMs, gamePageNextMs, modPageNextMs, paletteOpenAndLoadMs, reportPath }));
  expect(fixture.commands).toBe(1000); expect(remoteRequests).toEqual([]); expect(pageErrors).toEqual([]);
});
