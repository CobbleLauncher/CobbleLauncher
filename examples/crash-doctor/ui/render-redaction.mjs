// SPDX-License-Identifier: MIT
// Original synthetic logs and bridge. Does not assert real native clipboard/save-dialog acceptance.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
import { redactLog } from '../../../packages/plugin-web-sdk/src/redaction.ts';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const log = readFileSync(path.join(root, 'examples/crash-doctor/fixtures/synthetic-private.log'), 'utf8');
const redacted = redactLog(log).text;
const require = createRequire(new URL('../../../apps/desktop/package.json', import.meta.url)), { chromium } = require('@playwright/test');
const output = path.join(root, 'target/ui-evidence/crash-redaction'), dist = path.join(root, 'examples/crash-doctor/ui/dist'); mkdirSync(output, { recursive: true });
const server = createServer((request, response) => {
  try { const file = path.resolve(dist, decodeURIComponent((request.url ?? '/').split('?')[0]).slice(1)); if (!file.startsWith(dist + path.sep)) throw Error('outside bundle'); response.setHeader('Content-Type', file.endsWith('.js') ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : 'text/html'); response.end(readFileSync(file)); }
  catch { response.writeHead(404); response.end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser; const proof = { syntheticTransport: true, nativeClipboardOrSaveDialogTested: false, checks: [] };
try {
  browser = await chromium.launch({ headless: true, channel: 'msedge' });
  for (const narrow of [false, true]) {
    const width = narrow ? 520 : 1250, page = await browser.newPage({ viewport: { width, height: 1000 }, reducedMotion: 'reduce' }), errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.addInitScript(({ log, narrow }) => {
      window.__calls = []; window.__mode = 'normal';
      window.__COBBLE_PLUGIN_BRIDGE__ = { request: async (method, params) => {
        window.__calls.push({ method, params });
        if (method === 'context') return { locale: narrow ? 'zh-CN' : 'en', theme: narrow ? 'dark' : 'light', instanceId: 'synthetic', games: [], recentGames: [{ id: 'synthetic', name: 'Synthetic private log', version: '1.21.1', lastLaunchedAt: 1 }] };
        if (method === 'diagnostics.list') return window.__mode === 'empty' ? [] : [{ id: 'latest', name: 'logs/latest.log', size: log.length, modifiedAt: 1 }, { id: 'large', name: 'logs/large.log', size: 720000, modifiedAt: 1 }];
        if (method === 'diagnostics.read') return { id: params.diagnosticId, name: 'logs/' + params.diagnosticId + '.log', text: params.diagnosticId === 'large' ? 'userId=x\n'.repeat(80000) : log, revision: 'a'.repeat(64) };
        if (method === 'clipboard.writeText' || method === 'exports.text') { if (window.__mode === 'error') throw Error('SYNTHETIC_EXPORT_FAILED'); return method === 'exports.text' ? { saved: window.__mode !== 'cancel' } : null; }
        if (method === 'diagnostics.check' || method === 'diagnostics.pick') return { schema_version: 1, findings: [], tables: [] };
        if (method === 'report.cancel') return null; throw Error('Unexpected fixture method ' + method);
      } };
    }, { log, narrow });
    const button = label => page.getByRole('button', { name: label, exact: true });
    const shot = async state => { const screenshot = `${state}-${width}-${narrow ? 'dark-zh' : 'light-en'}.png`; assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false); await page.screenshot({ path: path.join(output, screenshot), fullPage: true }); proof.checks.push({ state, width, locale: narrow ? 'zh-CN' : 'en', theme: narrow ? 'dark' : 'light', screenshot, pageOverflow: false }); };
    await page.goto(`http://127.0.0.1:${server.address().port}/index.html`); await page.waitForFunction(() => !document.querySelector('#check').disabled); await page.locator('#share-tab').click();
    assert.equal(await page.locator('#source').isVisible(), false); assert(!(await page.locator('.redaction-output').innerText()).includes('SyntheticPlayer')); assert.match(await page.locator('.redaction-output').innerText(), /NoClassDefFoundError/);
    await shot('redacted');
    await button(narrow ? '变更行' : 'Changes').click(); assert(await page.locator('.redaction-output mark').count() > 0); await shot('changes');
    await button(narrow ? '读取文本' : 'Loaded text').click(); assert((await page.locator('.redaction-output').innerText()).includes('SyntheticPlayer')); await shot('loaded');
    await button(narrow ? '复制脱敏文本' : 'Copy redacted text').click(); assert.equal(await page.evaluate(() => window.__calls.findLast(call => call.method === 'clipboard.writeText').params.text), redacted);
    await button(narrow ? '脱敏文本' : 'Redacted').click(); const search = page.getByRole('searchbox'); await search.fill('NoClassDefFoundError'); assert.equal(await page.locator('.redaction-line').count(), 1);
    await button(narrow ? '导出脱敏日志' : 'Export redacted log').click(); const exported = await page.evaluate(() => window.__calls.findLast(call => call.method === 'exports.text').params); assert.equal(exported.text, redacted); assert.equal(exported.name, 'crash-doctor-redacted.log');
    await search.fill('No synthetic match'); await shot('search-empty'); await search.fill('');
    await page.evaluate(() => window.__mode = 'cancel'); await button(narrow ? '导出脱敏日志' : 'Export redacted log').click(); await page.waitForFunction(() => document.querySelector('.redaction-status').textContent === '');
    await page.evaluate(() => window.__mode = 'error'); await button(narrow ? '导出脱敏日志' : 'Export redacted log').click(); await page.waitForFunction(() => /Try again|请重试/.test(document.querySelector('.redaction-status').textContent)); await shot('export-error');
    await page.evaluate(() => window.__mode = 'normal'); await page.locator('#files button').filter({ hasText: 'logs/large.log' }).click(); await page.waitForFunction(() => document.querySelector('.redaction-limit').textContent.includes('1 MiB')); assert.equal(await button(narrow ? '导出脱敏日志' : 'Export redacted log').isDisabled(), true); assert.equal(await button(narrow ? '复制脱敏文本' : 'Copy redacted text').isDisabled(), true); await shot('output-limit');
    await page.evaluate(() => window.__mode = 'empty'); await page.locator('#refresh').click(); await page.waitForFunction(() => /Choose a game log|选择游戏日志/.test(document.querySelector('.redaction-output').textContent)); await shot('no-files');
    assert.deepEqual(errors, []); await page.close();
  }
  proof.passed = true; writeFileSync(path.join(output, 'render-redaction.json'), JSON.stringify(proof, null, 2) + '\n'); process.stdout.write(`Redaction fixture layouts/interactions passed: ${output}\n`);
} finally { await browser?.close(); await new Promise(resolve => server.close(resolve)); }
