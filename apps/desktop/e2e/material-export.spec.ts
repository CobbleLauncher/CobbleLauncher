// SPDX-License-Identifier: MIT
// The native picker and file write are mocked; this spec never opens a real dialog.
import { expect, test, type Page } from '@playwright/test';

type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & {
  __calls: Call[];
  __exportResult: boolean;
  __failExport: boolean;
};

const opened = {
  resourceId: 'opaque-schematic-resource',
  name: 'Test blueprint',
  scene: {
    width: 1,
    height: 1,
    length: 1,
    palette: ['minecraft:stone'],
    materials: [{ state: 'minecraft:stone', count: 1 }],
    chunk_count: 1,
  },
};

async function setup(page: Page) {
  await page.addInitScript((initialBlueprint) => {
    const target = window as unknown as Mock & {
      __TAURI_INTERNALS__: Record<string, unknown>;
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => void };
    };
    target.__calls = [];
    target.__exportResult = false;
    target.__failExport = false;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'get_blueprint_viewer_choices') return { viewer_id: 'synthetic-viewer', viewers: [{ id: 'synthetic-viewer', name: 'Fixture Viewer', file_extensions: ['schem', 'litematic'] }], disabled_viewers: [], interface: { id: 'launcher:viewer', version: '^1.0.0', implementations: [{ plugin_id: 'synthetic-viewer', name: 'Fixture Viewer', version: '1.0.0' }], candidates: [] } };
        if (command === 'list_games') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'list_tasks') return [];
        if (command === 'plugin:event|listen') return 1;
        if (command === 'list_blueprints') return [];
        if (command === 'open_schematic') return structuredClone(initialBlueprint);
        if (command === 'read_viewer_chunk') return { blocks: [{ x: 0, y: 0, z: 0, palette_index: 0 }] };
        if (command === 'export_blueprint_materials') {
          if (target.__failExport) throw new Error('synthetic export failure');
          return target.__exportResult;
        }
        return null;
      },
    };
  }, opened);
  await page.goto('/');
  await page.getByRole('button', { name: '市场', exact: true }).click();
  await page.getByRole('tab', { name: '投影', exact: true }).click();
  await page.getByRole('button', { name: '打开 .schem 蓝图', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Test blueprint' })).toBeVisible();
}

async function exportCalls(page: Page) {
  return page.evaluate(() => (window as unknown as Mock).__calls.filter(call => call.command === 'export_blueprint_materials'));
}

test('exports the open blueprint through its opaque resource id and keeps cancellation quiet', async ({ page }) => {
  await setup(page);
  await page.getByRole('button', { name: '导出全部材料（CSV）', exact: true }).click();
  await expect.poll(() => exportCalls(page)).toHaveLength(1);
  expect(await exportCalls(page)).toEqual([{ command: 'export_blueprint_materials', args: { resourceId: opened.resourceId } }]);
  await expect(page.getByRole('button', { name: '导出全部材料（CSV）', exact: true })).toBeEnabled();
  await expect(page.getByRole('heading', { name: 'Test blueprint' })).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByRole('status')).toHaveCount(0);

  await page.evaluate(() => { (window as unknown as Mock).__exportResult = true; });
  await page.getByRole('button', { name: '导出全部材料（CSV）', exact: true }).click();
  await expect.poll(() => exportCalls(page)).toHaveLength(2);
  await expect(page.getByRole('button', { name: '导出全部材料（CSV）', exact: true })).toBeEnabled();
  await expect(page.getByRole('heading', { name: 'Test blueprint' })).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByRole('status')).toHaveCount(0);

  await page.evaluate(() => { (window as unknown as Mock).__failExport = true; });
  await page.getByRole('button', { name: '导出全部材料（CSV）', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('synthetic export failure');
  await expect(page.getByRole('button', { name: '导出全部材料（CSV）', exact: true })).toBeEnabled();
  await expect(page.getByRole('heading', { name: 'Test blueprint' })).toBeVisible();
});
