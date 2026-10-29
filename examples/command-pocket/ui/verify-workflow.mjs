// SPDX-License-Identifier: MIT
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const require = createRequire(resolve('apps/desktop/package.json'));
const { createServer } = await import(pathToFileURL(require.resolve('vite')).href);
const { chromium } = require('@playwright/test');
const server = await createServer({ configFile: false, root: resolve('examples/command-pocket/ui'), server: { host: '127.0.0.1', port: 0 }, clearScreen: false });
await server.listen();
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const out = resolve('target/ui-evidence/t097-command-pocket');
await mkdir(out, { recursive: true });
const receipts = [];
try {
  for (const sample of [{ locale: 'en', theme: 'light', width: 1280 }, { locale: 'zh-CN', theme: 'dark', width: 560 }]) {
    const page = await browser.newPage({ viewport: { width: sample.width, height: 900 } });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(sample => {
      window.calls = []; window.clips = []; window.clipboardMode = 'ok';
      const entries = [{ id: 'home', name: 'Home', template: '/home {place}', favorite: true }, { id: 'spawn', name: 'Spawn', template: '/spawn', favorite: false }];
      window.__COBBLE_PLUGIN_BRIDGE__ = { request: async (method, params) => {
        window.calls.push({ method, params });
        if (method === 'context') return { ...sample, instanceId: 'game-a', games: [], inspectionGames: [{ id: 'game-a', name: 'Survival', version: '1.21.4' }] };
        if (method === 'inspection.snapshot') return { scope: 'servers', game: { id: 'game-a' }, data: { available: true, truncated: false, items: [{ serverId: 'a'.repeat(64), name: 'Friends', address: 'friends.invalid' }, { serverId: 'b'.repeat(64), name: 'Creative', address: 'creative.invalid' }] } };
        if (method === 'data.read') return { value: { schemaVersion: 1, entries }, revision: 'r1' };
        if (method === 'view.setDirty') return null;
        if (method === 'clipboard.writeText') {
          if (window.clipboardMode === 'fail') throw Error('Clipboard unavailable');
          if (window.clipboardMode === 'delay') await new Promise(resolve => { window.finishCopy = resolve; });
          window.clips.push(params.text); return null;
        }
        throw Error(method);
      } };
    }, sample);
    await page.goto(server.resolvedUrls.local[0]);
    const scope = page.locator('select').nth(1), search = page.getByRole('searchbox');
    const copy = page.locator('[data-action=copy-command]');
    const title = page.getByLabel(sample.locale === 'en' ? 'Name' : '名称', { exact: true });
    const param = page.getByLabel('place', { exact: true });
    await scope.selectOption('a'.repeat(64)); await param.fill('base'); await param.press('Enter');
    await page.waitForFunction(() => window.clips.length === 1);
    assert.equal(await copy.textContent(), sample.locale === 'en' ? 'Copied' : '已复制');
    assert.equal(await page.evaluate(() => window.clips.at(-1)), '/home base');
    await search.fill('spawn'); await search.press('Enter');
    assert.equal(await title.inputValue(), 'Spawn'); assert.equal(await copy.evaluate(button => button === document.activeElement), true);
    await search.fill('home');
    await search.dispatchEvent('keydown', { key: 'Enter', isComposing: true });
    assert.equal(await title.inputValue(), 'Spawn');
    await search.press('Enter'); assert.equal(await param.inputValue(), 'base');
    assert.equal(await param.evaluate(input => input === document.activeElement), true);
    await search.press('Escape'); assert.equal(await search.inputValue(), '');
    await search.press('ArrowDown'); await page.keyboard.press('ArrowDown'); await page.keyboard.press('Enter');
    assert.equal(await title.inputValue(), 'Spawn');
    await search.fill('home'); await page.getByRole('button', { name: sample.locale === 'en' ? 'Clear search' : '清空搜索', exact: true }).click();
    assert.equal(await search.inputValue(), ''); assert.equal(await search.evaluate(input => input === document.activeElement), true);
    await search.fill('home'); await search.press('Enter'); await title.fill('Protected draft');
    await search.fill('spawn'); await search.press('Enter');
    assert.equal(await page.locator('.notice').isVisible(), true);
    await page.getByRole('button', { name: sample.locale === 'en' ? 'Cancel' : '取消', exact: true }).click();
    assert.equal(await title.inputValue(), 'Protected draft');
    await page.getByRole('button', { name: sample.locale === 'en' ? 'Discard edits' : '放弃修改', exact: true }).click();
    await search.fill(''); await param.fill('base2');
    await page.evaluate(() => { window.clipboardMode = 'fail'; }); await param.press('Enter');
    await page.locator('.editor>.error').waitFor({ state: 'visible' });
    assert.equal(await copy.textContent(), sample.locale === 'en' ? 'Retry copy' : '重试复制');
    assert.equal(await page.evaluate(() => window.clips.length), 1);
    await page.screenshot({ path: resolve(out, `retry-${sample.locale}-${sample.width}.png`), fullPage: true });
    await page.evaluate(() => { window.clipboardMode = 'ok'; }); await copy.click();
    await page.waitForFunction(() => window.clips.length === 2);
    assert.equal(await page.locator('.editor>.error').isHidden(), true);
    await page.evaluate(() => { window.clipboardMode = 'delay'; }); await copy.click();
    await page.waitForFunction(() => typeof window.finishCopy === 'function');
    const started = await page.evaluate(() => window.calls.filter(call => call.method === 'clipboard.writeText').length);
    await param.press('Enter');
    assert.equal(await page.evaluate(() => window.calls.filter(call => call.method === 'clipboard.writeText').length), started);
    await search.fill('spawn'); await search.press('Enter');
    await page.evaluate(() => { window.finishCopy(); window.clipboardMode = 'ok'; });
    await page.waitForFunction(() => window.clips.length === 3);
    assert.equal(await copy.textContent(), sample.locale === 'en' ? 'Copy command' : '复制指令');
    await search.fill('home'); await search.press('Enter'); assert.equal(await param.inputValue(), 'base2');
    await scope.selectOption('b'.repeat(64)); assert.equal(await param.inputValue(), '');
    await scope.selectOption('a'.repeat(64)); assert.equal(await param.inputValue(), 'base2');
    await param.press('Enter'); await page.waitForFunction(() => window.clips.length === 4);
    assert.equal(await page.evaluate(() => window.calls.some(call => call.method === 'data.write')), false);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    assert.deepEqual(errors, []);
    await page.screenshot({ path: resolve(out, `commands-${sample.locale}-${sample.width}.png`), fullPage: true });
    receipts.push({ ...sample, keyboardSearchAndCopy: true, compositionPreserved: true, dirtyGuard: true, parameterRecall: true, serverIsolation: true, failureRetry: true, duplicateCopyGuard: true, staleSuccessIgnored: true, noDataWrites: true });
    await page.close();
  }
  await writeFile(resolve(out, 'summary.json'), JSON.stringify({ evidence: 'Synthetic bridge/source UI, no OS clipboard or server connection acceptance', receipts }, null, 2));
  console.log(JSON.stringify({ passed: receipts.length, out }));
} finally { await browser.close(); await server.close(); }
