// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import InspectorPanel from './InspectorPanel.vue';
import { setLocale } from './i18n';
import { isInspectorPlugin } from './plugin-reports';
import { isSupportedCatalogPlugin } from './plugin-catalogs';
import ToolsPage from './ToolsPage.vue';
import type { InspectorPageReply, InspectorQuery, InspectorSession } from './inspector/types';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); vi.useRealTimers(); setLocale('zh-CN'); });
const cell = (text: string, complete = true) => ({ text, byte_length: new TextEncoder().encode(text).length, complete });
const summary = { name: cell('Pack'), version_id: cell('1'), declared_files: 120, override_files: 2, declared_download_bytes: 4096 };
const session: InspectorSession = { sessionId: 'opaque-session', summary: { current: summary, baseline: summary, changes: { added: 1, removed: 2, changed: 3, unchanged: 116 } } };
function reply(args: Record<string, unknown>, text = '<script>row</script>'): InspectorPageReply {
  const query = args.query as InspectorQuery;
  return { sessionId: session.sessionId, revision: Number(args.revision), page: { query, total_rows: 122, matched_rows: query.search ? 1 : 122, next_offset: query.offset === 0 && !query.search ? 50 : null, rows: [{ id: 'opaque-row', path: cell('mods/item.jar'), source: 'declared_file', change: null, before: null, after: cell(text, false) }] }, report: { schema_version: 1, findings: [], tables: [] } };
}
async function open() {
  setLocale('en-US'); const wrapper = mount(InspectorPanel, { props: { pluginId: 'custom.inspector' } });
  await wrapper.get('.secondary-button').trigger('click'); await flushPromises(); return wrapper;
}
it('uses manifest opt-in rather than a plugin id', () => {
  const manifest = { api_version: '0.1.0', capabilities: ['report'], file_extensions: ['json'], report_protocols: ['cobble.mrpack-page@1'] };
  expect(isInspectorPlugin(manifest)).toBe(true);
  expect(isInspectorPlugin({ ...manifest, report_protocols: [] })).toBe(false);
  expect(isInspectorPlugin({ ...manifest, capabilities: ['viewer'] })).toBe(false);
});
it('pages the whole server dataset at 50 rows and retries failed pages without repicking', async () => {
  let failPage = true;
  vi.mocked(invoke).mockImplementation(async (command, rawArgs = {}) => {
    const args = rawArgs as Record<string, unknown>;
    if (command === 'begin_pack_inspection') return session;
    if (command === 'query_pack_inspection') {
      if ((args.query as InspectorQuery).offset === 50 && failPage) { failPage = false; throw new Error('Page failed'); }
      return reply(args);
    }
    return null;
  });
  const wrapper = await open();
  expect(invoke).toHaveBeenCalledWith('begin_pack_inspection', { pluginId: 'custom.inspector', compare: false, locale: 'en' });
  expect(wrapper.text()).toContain('Items 1–1 of 122'); expect(wrapper.findAll('tbody tr')).toHaveLength(1);
  expect(wrapper.findAll('tbody script')).toHaveLength(0);
  await wrapper.findAll('button').find(b => b.text() === 'Next page')!.trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Page failed');
  await wrapper.get('[role="alert"] button').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('Items 51–51 of 122');
  const queries = vi.mocked(invoke).mock.calls.filter(([command]) => command === 'query_pack_inspection');
  expect(queries.map(([,args]) => ((args as Record<string, unknown>).query as InspectorQuery).limit)).toEqual([50, 50, 50]);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'begin_pack_inspection')).toHaveLength(1);
  wrapper.unmount(); expect(invoke).toHaveBeenCalledWith('close_pack_inspection', { sessionId: 'opaque-session' });
});
it('supersedes stale searches and applies totals from the complete filtered dataset', async () => {
  vi.useFakeTimers();
  let finishOld!: (value: unknown) => void; let oldArgs!: Record<string, unknown>;
  vi.mocked(invoke).mockImplementation((command, rawArgs = {}) => {
    const args = rawArgs as Record<string, unknown>;
    if (command === 'begin_pack_inspection') return Promise.resolve(session);
    if (command === 'query_pack_inspection') {
      if ((args.query as InspectorQuery).search === 'old') { oldArgs = args; return new Promise(resolve => { finishOld = resolve; }); }
      return Promise.resolve(reply(args, (args.query as InspectorQuery).search || 'Initial'));
    }
    return Promise.resolve(null);
  });
  const wrapper = await open(); const input = wrapper.get('input');
  await input.setValue('old'); await vi.advanceTimersByTimeAsync(250); await flushPromises();
  await input.setValue('needle beyond first page'); await vi.advanceTimersByTimeAsync(250); await flushPromises();
  expect(wrapper.text()).toContain('needle beyond first page'); expect(wrapper.text()).toContain('Items 1–1 of 1'); expect(wrapper.text()).toContain('122 total items');
  finishOld(reply(oldArgs, 'Stale result')); await flushPromises(); expect(wrapper.text()).not.toContain('Stale result');
  expect((vi.mocked(invoke).mock.calls.filter(([command]) => command === 'query_pack_inspection').at(-1)?.[1] as Record<string, unknown>).query).toEqual({ section: 'overview', search: 'needle beyond first page', offset: 0, limit: 50 });
  wrapper.unmount();
});
it('reads full metadata in exact byte chunks and renders all untrusted text as plaintext', async () => {
  vi.mocked(invoke).mockImplementation(async (command, rawArgs = {}) => {
    const args = rawArgs as Record<string, unknown>;
    if (command === 'begin_pack_inspection') return session;
    if (command === 'query_pack_inspection') return reply(args);
    if (command === 'detail_pack_inspection') {
      const query = args.query as { offset: number };
      return { sessionId: session.sessionId, revision: args.revision, detail: { row_id: 'opaque-row', field: 'after', total_bytes: 8, offset: query.offset, next_offset: query.offset === 0 ? 4 : null, text: query.offset === 0 ? '<img' : '>end' } };
    }
    return null;
  });
  const wrapper = await open();
  await wrapper.findAll('button').find(b => b.text() === 'Read full value')!.trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(b => b.text() === 'Read more')!.trigger('click'); await flushPromises();
  expect(wrapper.get('pre').text()).toBe('<img>end'); expect(wrapper.findAll('img')).toHaveLength(0);
  const args = vi.mocked(invoke).mock.calls.filter(([command]) => command === 'detail_pack_inspection').map(([,args]) => (args as Record<string, unknown>).query);
  expect(args).toEqual([{ row_id: 'opaque-row', field: 'after', offset: 0, limit: 2048 }, { row_id: 'opaque-row', field: 'after', offset: 4, limit: 2048 }]);
  expect(wrapper.findAll('button').some(b => b.text() === 'Read more')).toBe(false); wrapper.unmount();
});

it('cancels a pending picker session and discards its late result', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'begin_pack_inspection' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(null));
  setLocale('en-US'); const wrapper = mount(InspectorPanel, { props: { pluginId: 'custom.inspector' } });
  await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(b => b.text() === 'Cancel')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('close_pack_inspection', { pluginId: 'custom.inspector' });
  finish(session); await flushPromises();
  expect(wrapper.find('input').exists()).toBe(false);
  expect(invoke).toHaveBeenCalledWith('close_pack_inspection', { sessionId: session.sessionId });
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'query_pack_inspection')).toHaveLength(0);
  wrapper.unmount();
});

it('routes opted-in report plugins to Inspector while preserving ordinary report plugins', async () => {
  const manifest = { id: 'custom.report', name: 'Custom report', version: '1', api_version: '0.1.0', publisher: 'Publisher', capabilities: ['report'], file_extensions: ['json'], component_sha256: 'a'.repeat(64) };
  expect(isSupportedCatalogPlugin({ manifest: { ...manifest, report_protocols: ['unknown'] }, description: '', component_url: '' })).toBe(false);
  expect(isSupportedCatalogPlugin({ manifest: { ...manifest, report_protocols: ['cobble.mrpack-page@1'] }, description: '', component_url: '' })).toBe(true);
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'list_plugins' ? [{ manifest, enabled: true }, { manifest: { ...manifest, id: 'custom.inspector', report_protocols: ['cobble.mrpack-page@1'] }, enabled: true }] : []));
  const wrapper = mount(ToolsPage, { props: { selectedPluginId: 'custom.inspector' }, global: { stubs: { InspectorPanel: true, PluginReportPanel: true, BlueprintsPage: true } } }); await flushPromises();
  expect(wrapper.findAll('inspector-panel-stub')).toHaveLength(1);
  expect(wrapper.get('inspector-panel-stub').attributes('pluginid')).toBe('custom.inspector');
  expect(wrapper.get('inspector-panel-stub').attributes('allowcompare')).toBe('false');
  await wrapper.setProps({ selectedPluginId: 'custom.report' });
  expect(wrapper.findAll('plugin-report-panel-stub')).toHaveLength(1);
  expect(wrapper.findAll('inspector-panel-stub')).toHaveLength(0);
  wrapper.unmount();
});

it('does not close another plugin session when an inactive panel unmounts', () => {
  const wrapper = mount(InspectorPanel, { props: { pluginId: 'inactive.inspector' } });
  wrapper.unmount();
  expect(invoke).not.toHaveBeenCalled();
});
