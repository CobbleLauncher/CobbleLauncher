// SPDX-License-Identifier: MIT
// Synthetic evidence only. Actual rollback/config confirmation is verified by host/native tests.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const require = createRequire(new URL('../../../apps/desktop/package.json', import.meta.url)), { chromium } = require('@playwright/test');
const output = path.join(root, 'target/ui-evidence/crash-recovery'), dist = path.join(root, 'examples/crash-doctor/ui/dist'); mkdirSync(output, { recursive: true });
const server = createServer((request, response) => {
  try { const file = path.resolve(dist, decodeURIComponent((request.url ?? '/').split('?')[0]).slice(1)); if (!file.startsWith(dist + path.sep)) throw Error('outside bundle'); response.setHeader('Content-Type', file.endsWith('.js') ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : 'text/html'); response.end(readFileSync(file)); }
  catch { response.writeHead(404); response.end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser; const proof = { syntheticTransport: true, nativeRollbackExecuted: false, checks: [] };
try {
  browser = await chromium.launch({ headless: true, channel: 'msedge' });
  for (const narrow of [false, true]) {
    const width = narrow ? 520 : 1250, page = await browser.newPage({ viewport: { width, height: 1000 }, reducedMotion: 'reduce' }), errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.addInitScript(({ narrow }) => {
      window.__calls = []; window.__mode = 'normal';
      const runtime = { javaMode: 'automatic', selectedJavaMajor: 21, memoryMb: 4096, jvmArgumentCount: 2 }, base = Date.UTC(2026, 9, 23, 10);
      const snapshot = gameId => ({ schemaVersion: 1, gameId, revision: 'a'.repeat(64), baselineAvailable: true, lastCleanLaunch: window.__mode === 'unknown' ? null : { startedAt: base, endedAt: base + 600000, runtime: { ...runtime, memoryMb: 2048 } }, runtime: { ...runtime, changedSinceClean: window.__mode === 'unknown' ? null : true }, launchState: window.__mode === 'running' ? 'running' : 'idle', mods: { available: true, truncated: false, items: [{ id: 'install-new', createdAt: base + 1200000, status: 'applied', files: [{ name: 'Adventure Atlas', before: 'adventure-atlas-1.0.jar', after: 'adventure-atlas-1.1.jar' }, { name: 'Atlas Library', before: null, after: 'atlas-library-2.0.jar' }] }, { id: 'install-old', createdAt: base - 1200000, status: 'undone', files: [{ name: 'Older Mod', before: 'old.jar', after: null }] }] }, configs: { available: true, truncated: false, files: [{ id: 'c'.repeat(64), name: 'adventure-atlas/client.toml', backups: [{ id: '100-' + 'b'.repeat(64), createdAt: base + 900000 }] }] } });
      window.__COBBLE_PLUGIN_BRIDGE__ = { request: async (method, params) => {
        window.__calls.push({ method, params });
        if (method === 'context') return { locale: narrow ? 'zh-CN' : 'en', theme: narrow ? 'dark' : 'light', instanceId: 'synthetic', games: [], recentGames: [{ id: 'synthetic', name: narrow ? '冒险世界' : 'Adventure world', version: '1.21.1', lastLaunchedAt: 1 }] };
        if (method === 'diagnostics.list') return [{ id: 'latest', name: 'logs/latest.log', size: 100, modifiedAt: 1 }];
        if (method === 'diagnostics.read') return { id: 'latest', name: 'logs/latest.log', text: '[12:00:00] Synthetic launch failed', revision: 'a'.repeat(64) };
        if (method === 'recovery.snapshot') { if (window.__mode === 'error') throw Error('RECOVERY_UNAVAILABLE'); const value = snapshot(params.gameId); if (window.__mode === 'empty') { value.mods.items = []; value.configs.files = []; } return value; }
        if (method === 'recovery.openRequest') { if (window.__mode === 'stale') throw Error('RECOVERY_SNAPSHOT_CHANGED'); return { requested: true }; }
        if (method === 'report.cancel') return null; throw Error('Unexpected fixture method ' + method);
      } };
    }, { narrow });
    const button = label => page.getByRole('button', { name: label, exact: true });
    const shot = async state => { const screenshot = `${state}-${width}-${narrow ? 'dark-zh' : 'light-en'}.png`; assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false); await page.screenshot({ path: path.join(output, screenshot), fullPage: true }); proof.checks.push({ state, width, locale: narrow ? 'zh-CN' : 'en', theme: narrow ? 'dark' : 'light', screenshot, pageOverflow: false }); };
    const refresh = async mode => { await page.evaluate(value => window.__mode = value, mode); await page.locator('#refresh').click(); await page.waitForFunction(() => document.querySelector('#recovery-panel').getAttribute('aria-busy') === 'false'); };
    await page.goto(`http://127.0.0.1:${server.address().port}/index.html`); await page.waitForFunction(() => !document.querySelector('#check').disabled);
    assert.equal(await page.evaluate(() => window.__calls.some(call => call.method === 'recovery.snapshot')), false);
    await page.locator('#recovery-tab').click(); await page.waitForFunction(() => document.querySelectorAll('.recovery-events>li').length === 2);
    assert(!(await page.locator('.recovery-events').innerText()).includes('Older Mod')); await shot('history');
    await button(narrow ? '预览撤销' : 'Review undo').click(); const operation = await page.evaluate(() => window.__calls.findLast(call => call.method === 'recovery.openRequest').params); assert.deepEqual(operation, { gameId: 'synthetic', revision: 'a'.repeat(64), target: { kind: 'modUndo', operationId: 'install-new' } });
    await button(narrow ? '查看备份' : 'Review backup').click(); assert.equal(await page.evaluate(() => window.__calls.findLast(call => call.method === 'recovery.openRequest').params.target.kind), 'configRestore');
    const search = page.getByRole('searchbox'); await search.fill('not a matching change'); await shot('search-empty'); await search.fill('');
    await refresh('stale'); await button(narrow ? '检查运行设置' : 'Review runtime settings').click(); await page.waitForFunction(() => /History changed|记录已变化/.test(document.querySelector('.recovery-status').textContent)); await shot('stale');
    await refresh('unknown'); assert.match(await page.locator('.recovery-baseline').innerText(), narrow ? /尚无正常结束记录/ : /No recorded normal exit/); assert.equal(await page.locator('.recovery-events>li').count(), 3); await shot('unknown-baseline');
    await refresh('running'); assert.equal(await button(narrow ? '预览撤销' : 'Review undo').isDisabled(), true); await shot('running');
    await refresh('empty'); await shot('no-history'); await refresh('error'); await shot('read-error');
    assert.deepEqual(errors, []); await page.close();
  }
  proof.passed = true; writeFileSync(path.join(output, 'render-recovery.json'), JSON.stringify(proof, null, 2) + '\n'); process.stdout.write(`Recovery fixture layouts/interactions passed: ${output}\n`);
} finally { await browser?.close(); await new Promise(resolve => server.close(resolve)); }
