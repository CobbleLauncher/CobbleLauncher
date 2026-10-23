// SPDX-License-Identifier: MIT
// Tauri boundary only; no native picker, network access, or Wasm execution.
import { expect, test, type Page } from '@playwright/test';

type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & {
  __calls: Call[];
  __analysisMode: 'picker-cancel' | 'pending' | 'report';
  __finishAnalysis?: (value: unknown) => void;
};

const manifest = {
  id: 'example-log-report',
  name: 'Log Report',
  version: '1.0.0',
  api_version: '0.1.0',
  publisher: 'Example Tools',
  capabilities: ['report'],
  file_extensions: ['log', 'txt', 'json'],
  component_sha256: 'a'.repeat(64),
};
const plugin = { manifest, enabled: true };
const report = {
  schema_version: 1,
  findings: [{
    code: 'unsafe-text',
    severity: 'warning',
    title: '<img src=x onerror=alert(1)>',
    message: '<script>window.__reportXss=1</script>',
    evidence: [{ input_index: 0, first_line: 2, last_line: 4 }],
    suggestions: ['<a href="file:///secret">Review this line</a>'],
  }],
  tables: [{ columns: ['Value'], rows: [['<iframe src="file:///secret">text</iframe>']] }],
};

async function setup(page: Page) {
  await page.addInitScript(({ plugin, report }) => {
    const target = window as unknown as Mock & {
      isTauri: boolean;
      __TAURI_INTERNALS__: unknown;
      __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
    };
    target.isTauri = true;
    target.__calls = [];
    target.__analysisMode = 'picker-cancel';
    let callbackId = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callbackId,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') return callbackId;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'list_plugins') return [plugin];
        if (command === 'list_plugin_catalogs' || command === 'list_bundled_plugins') return [];
        if (command === 'analyze_plugin_report') {
          if (target.__analysisMode === 'picker-cancel') return null;
          if (target.__analysisMode === 'pending') {
            return await new Promise((resolve) => { target.__finishAnalysis = resolve; });
          }
          return report;
        }
        if (command === 'cancel_plugin_report') {
          target.__finishAnalysis?.(null);
          target.__finishAnalysis = undefined;
          return null;
        }
        return null;
      },
    };
  }, { plugin, report });
  await page.goto('/');
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('tab', { name: '扩展', exact: true }).click();
  await expect(page.getByRole('heading', { name: '插件目录', exact: true })).toBeVisible();
  await expect(page.locator('.plugin-report-panel')).toBeVisible();
}

async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}

test('report analysis handles picker cancel and running cancel, then renders the retry as plain text', async ({ page }) => {
  const remoteRequests: string[] = [];
  page.on('request', (request) => {
    if (new URL(request.url()).hostname !== '127.0.0.1') remoteRequests.push(request.url());
  });
  await setup(page);
  expect((await calls(page, 'list_plugins'))[0].args).toEqual({});
  for (const command of ['analyze_plugin_report', 'cancel_plugin_report']) expect(await calls(page, command)).toHaveLength(0);

  await page.getByRole('button', { name: '分析文件…', exact: true }).click();
  await expect.poll(async () => (await calls(page, 'analyze_plugin_report')).length).toBe(1);
  await expect(page.locator('.report-results')).toHaveCount(0);
  await expect(page.getByRole('alert')).toHaveCount(0);

  await page.evaluate(() => { (window as unknown as Mock).__analysisMode = 'pending'; });
  await page.getByRole('button', { name: '分析文件…', exact: true }).click();
  await expect(page.getByRole('button', { name: '正在分析…', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect.poll(async () => (await calls(page, 'cancel_plugin_report')).length).toBe(1);
  await expect(page.locator('.report-results')).toHaveCount(0);
  await expect(page.getByRole('button', { name: '分析文件…', exact: true })).toBeEnabled();

  await page.evaluate(() => { (window as unknown as Mock).__analysisMode = 'report'; });
  await page.getByRole('button', { name: '分析文件…', exact: true }).click();
  await expect(page.locator('.report-finding h3')).toContainText(report.findings[0]!.title);
  await expect(page.locator('.report-finding')).toContainText(report.findings[0]!.message);
  await expect(page.locator('.report-evidence')).toContainText('文件 1，第 2–4 行');
  await expect(page.locator('.report-table-scroll')).toContainText(report.tables[0]!.rows[0]![0]!);
  await expect(page.locator('.report-results img, .report-results script, .report-results a, .report-results iframe')).toHaveCount(0);
  expect(await page.evaluate(() => (window as unknown as Window & { __reportXss?: number }).__reportXss)).toBeUndefined();
  expect(remoteRequests).toEqual([]);
  await page.setViewportSize({ width: 560, height: 720 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  expect((await calls(page, 'analyze_plugin_report')).map((call) => call.args)).toEqual([
    { pluginId: manifest.id }, { pluginId: manifest.id }, { pluginId: manifest.id },
  ]);
});
