// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import OnlineBlueprints from './OnlineBlueprints.vue';
import type { Game } from './games';
import type { OnlineBlueprint, OnlineBlueprintOutput } from './online-blueprints';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
const game = { id: 'game-current', name: 'Builder', version: '1.21.1', status: 'installed' } as Game;
const item: OnlineBlueprint = { id: '11111111-1111-4111-8111-111111111111', title: 'Synthetic Planter', description: 'A plain text description', version: 1, format: 'schem', author: 'Synthetic Author', attribution: 'repost', license: null, java_release: '1.21.11', namespaces: ['minecraft'], preview_url: null };
const page: OnlineBlueprintOutput = { items: [item], next_cursor: null, detail: null, download: null };
const singleChoices = { viewer_id: 'compatible-viewer', viewers: [{ id: 'compatible-viewer', name: 'Compatible Viewer', file_extensions: ['schem', 'litematic'] }], interface: { id: 'launcher:viewer', version: '^1.0.0', implementations: [{ plugin_id: 'compatible-viewer', name: 'Compatible Viewer', version: '1.0.0' }], candidates: [] } };
beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => command === 'get_blueprint_viewer_choices' ? singleChoices : command === 'get_blueprint_provider_availability' ? { available: true, provider_id: 'mcs-provider', providers: [{ id: 'mcs-provider', name: 'MCS', origins: ['https://www.mcschematic.top'] }], viewer_available: true } : command === 'online_blueprint_details' ? { ...page, detail: item, download: null } : page);
});
afterEach(() => { vi.clearAllMocks(); setLocale('zh-CN'); });
const props = { games: [game], initialGameId: game.id };

it.each(['MIT', null])('renders the detail license exactly and keeps an unknown license unknown (%s)', async license => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_blueprint_viewer_choices') return singleChoices;
    if (command === 'get_blueprint_provider_availability') return { available: true, provider_id: 'community-provider', providers: [{ id: 'community-provider', name: 'Community', origins: ['https://community.example'] }], viewer_available: true };
    if (command === 'online_blueprint_details') return { ...page, detail: { ...item, license } };
    return { ...page, items: [{ ...item, license: 'MIT' }] };
  });
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.findAll('button').find(button => button.attributes('aria-label') === item.title)!.trigger('click'); await flushPromises();
  const detail = wrapper.get('.online-detail').text();
  expect(detail).toContain(license ?? 'License not provided');
  if (license === null) expect(detail).not.toContain('MIT');
  wrapper.unmount();
});

it('requires a choice for multiple providers and keeps their requests and late results isolated', async () => {
  let finish: (value: unknown) => void = () => {};
  const providers = [{ id: 'mcs-provider', name: 'MCS', origins: ['https://www.mcschematic.top'] }, { id: 'community-provider', name: 'Community', origins: ['https://community.example'] }];
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'get_blueprint_viewer_choices') return singleChoices;
    if (command === 'get_blueprint_provider_availability') return { available: true, provider_id: null, providers, viewer_available: true };
    if (command === 'search_online_blueprints' && (args as { providerId: string }).providerId === 'community-provider') return new Promise(resolve => { finish = resolve; });
    return page;
  });
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined();
  await wrapper.findAll('select')[0]!.setValue('community-provider');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('search_online_blueprints', expect.objectContaining({ providerId: 'community-provider', gameId: game.id }));
  await wrapper.findAll('select')[0]!.setValue('mcs-provider');
  finish({ ...page, items: [{ ...item, id: 'community-hut-a', title: 'Community Hut' }] }); await flushPromises();
  expect(wrapper.text()).not.toContain('Community Hut');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('search_online_blueprints', expect.objectContaining({ providerId: 'mcs-provider', gameId: game.id }));
  expect(wrapper.text()).toContain(item.title);
  expect(invoke).toHaveBeenCalledWith('cancel_blueprint_provider', { requestId: expect.any(String) });
  wrapper.unmount();
});

it('checks the installed provider without performing an automatic public search', async () => {
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith('get_blueprint_provider_availability');
  expect(wrapper.find('.online-search-scope').exists()).toBe(false);
  wrapper.unmount();
});

it('shows a real unavailable state when the provider is disabled or missing', async () => {
  vi.mocked(invoke).mockResolvedValue({ available: false, provider_id: null, providers: [], viewer_available: true });
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  expect(wrapper.text()).toContain('Install and enable a blueprint provider first.');
  expect(wrapper.find('form').exists()).toBe(false);
  await wrapper.findAll('button').find(button => button.text() === 'Manage plugins')!.trigger('click');
  expect(wrapper.emitted('extensions')).toHaveLength(1);
  wrapper.unmount();
});

it('binds search to a registered game and preserves visible release and missing license information', async () => {
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  await wrapper.get('input[type="search"]').setValue('盆栽 & q=other');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('search_online_blueprints', expect.objectContaining({ providerId: 'mcs-provider', gameId: game.id, query: '盆栽 & q=other', format: null, modNamespace: null, cursor: null, requestId: expect.any(String) }));
  expect(wrapper.text()).toContain('1.21.11');
  await wrapper.findAll('button').find(button => button.attributes('aria-label') === item.title)!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('online_blueprint_details', expect.objectContaining({ id: item.id, version: 1 }));
  expect(wrapper.get('.online-detail').text()).toContain('Repost · License not provided');
  wrapper.unmount();
});

it('releases a late Viewer resource when its native request was cancelled', async () => {
  let finish: (value: unknown) => void = () => {};
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_blueprint_viewer_choices') return singleChoices;
    if (command === 'get_blueprint_provider_availability') return { available: true, provider_id: 'mcs-provider', providers: [{ id: 'mcs-provider', name: 'MCS', origins: ['https://www.mcschematic.top'] }], viewer_available: true };
    if (command === 'open_online_blueprint') return new Promise(resolve => { finish = resolve; });
    if (command === 'online_blueprint_details') return { ...page, detail: item };
    return page;
  });
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.findAll('button').find(button => button.attributes('aria-label') === item.title)!.trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Preview')!.trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Cancel')!.trigger('click');
  finish({ resourceId: 'owned-native-resource' }); await flushPromises();
  expect(wrapper.emitted('opened')).toBeUndefined();
  expect(invoke).toHaveBeenCalledWith('close_viewer', { resourceId: 'owned-native-resource' });
  expect(invoke).toHaveBeenCalledWith('cancel_blueprint_provider', { requestId: expect.any(String) });
  wrapper.unmount();
});

it('does not enable current Viewer preview for an unsupported provider format', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_blueprint_viewer_choices' ? singleChoices : command === 'get_blueprint_provider_availability' ? { available: true, provider_id: 'mcs-provider', providers: [{ id: 'mcs-provider', name: 'MCS', origins: ['https://www.mcschematic.top'] }], viewer_available: true } : command === 'online_blueprint_details' ? { ...page, detail: { ...item, format: 'create' } } : page);
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.findAll('button').find(button => button.attributes('aria-label') === item.title)!.trigger('click'); await flushPromises();
  const preview = wrapper.findAll('button').find(button => button.text() === 'Preview')!;
  expect(preview.attributes('disabled')).toBeDefined();
  expect(wrapper.text()).toContain('Preview currently supports .schem and .litematic only.');
  wrapper.unmount();
});




it('offers subscribed compatible provider candidates without choosing or installing one', async () => {
  vi.mocked(invoke).mockResolvedValue({ available: false, provider_id: null, providers: [], viewer_available: true, interface: { id: 'launcher:blueprint-provider', version: '^0.2.0', implementations: [], candidates: [{ source_id: 'subscription-a', plugin_id: 'community-provider', name: 'Community catalog', version: '1.4.0' }, { source_id: 'subscription-b', plugin_id: 'other-provider', name: 'Another catalog', version: '2.0.0' }] } });
  const wrapper = mount(OnlineBlueprints, { props }); await flushPromises();
  expect(wrapper.findAll('.provider-candidates button')).toHaveLength(2);
  expect(wrapper.find('form').exists()).toBe(false);
  expect(invoke).toHaveBeenCalledExactlyOnceWith('get_blueprint_provider_availability');
  await wrapper.findAll('.provider-candidates button')[0]!.trigger('click');
  expect(wrapper.emitted('extensions')?.[0]).toEqual([{ sourceId: 'subscription-a', pluginId: 'community-provider' }]);
  expect(invoke).not.toHaveBeenCalledWith('preview_catalog_plugin', expect.anything());
  wrapper.unmount();
});

async function openDetail(wrapper: ReturnType<typeof mount>) {
  await flushPromises();
  await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.findAll('button').find(button => button.attributes('aria-label') === item.title)!.trigger('click'); await flushPromises();
}

it('keeps preview optional while making a saved library copy the primary action', async () => {
  const wrapper = mount(OnlineBlueprints, { props });
  await openDetail(wrapper);
  expect(wrapper.find('.viewer-picker').exists()).toBe(false);
  await wrapper.findAll('button').find(button => button.text() === 'Save to library')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('download_online_blueprint', expect.objectContaining({ providerId: 'mcs-provider', id: item.id, version: item.version, requestId: expect.any(String) }));
  expect(wrapper.emitted('saved')).toHaveLength(1);
  expect(wrapper.text()).toContain('View library');
  await wrapper.findAll('button').find(button => button.text() === 'View library')!.trigger('click');
  expect(wrapper.emitted('library')).toHaveLength(1);
  await wrapper.findAll('button').find(button => button.text() === 'Preview')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_blueprint_viewer_choices', { providerId: 'mcs-provider', format: 'schem' });
  expect(invoke).toHaveBeenCalledWith('open_online_blueprint', expect.objectContaining({ viewerId: 'compatible-viewer' }));
  wrapper.unmount();
});

it('requires a choice between multiple compatible viewers and passes the exact selected ID', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_blueprint_provider_availability') return { available: true, provider_id: 'community-provider', providers: [{ id: 'community-provider', name: 'Community', origins: [] }], viewer_available: true };
    if (command === 'get_blueprint_viewer_choices') return { ...singleChoices, viewer_id: null, viewers: [{ id: 'first-viewer', name: 'First viewer', file_extensions: ['schem'] }, { id: 'other-viewer', name: 'Another viewer', file_extensions: ['schem'] }] };
    if (command === 'online_blueprint_details') return { ...page, detail: item };
    return page;
  });
  const wrapper = mount(OnlineBlueprints, { props });
  await openDetail(wrapper);
  const preview = wrapper.findAll('button').find(button => button.text() === 'Preview')!;
  expect(preview.attributes('disabled')).toBeDefined();
  await wrapper.get('.viewer-picker select').setValue('other-viewer');
  expect(preview.attributes('disabled')).toBeUndefined();
  await preview.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_online_blueprint', expect.objectContaining({ providerId: 'community-provider', viewerId: 'other-viewer' }));
  wrapper.unmount();
});

it.each([{ disabled: [] }, { disabled: [{ id: 'disabled-viewer', name: 'Disabled Viewer', file_extensions: ['schem'] }] }])('routes a missing or disabled viewer to Extensions without an installation prompt (%j)', async ({ disabled }) => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_blueprint_provider_availability') return { available: true, provider_id: 'community-provider', providers: [{ id: 'community-provider', name: 'Community', origins: [] }], viewer_available: false };
    if (command === 'get_blueprint_viewer_choices') return { ...singleChoices, viewer_id: null, viewers: [], disabled_viewers: disabled };
    if (command === 'online_blueprint_details') return { ...page, detail: item };
    return page;
  });
  const wrapper = mount(OnlineBlueprints, { props });
  await openDetail(wrapper);
  expect(wrapper.findAll('button').find(button => button.text() === 'Preview')!.attributes('disabled')).toBeDefined();
  expect(wrapper.get('.online-detail').text()).toContain(disabled.length ? 'Disabled: Disabled Viewer' : 'No compatible viewer is available.');
  await wrapper.findAll('button').find(button => button.text() === 'Manage plugins')!.trigger('click');
  expect(wrapper.emitted('extensions')).toHaveLength(1);
  expect(invoke).not.toHaveBeenCalledWith('open_online_blueprint', expect.anything());
  wrapper.unmount();
});

it('ignores late viewer choices from an earlier selected resource', async () => {
  const other = { ...item, id: '22222222-2222-4222-8222-222222222222', title: 'Another Planter' };
  const choices: ((value: unknown) => void)[] = [];
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'get_blueprint_provider_availability') return { available: true, provider_id: 'community-provider', providers: [{ id: 'community-provider', name: 'Community', origins: [] }], viewer_available: true };
    if (command === 'get_blueprint_viewer_choices') return new Promise(resolve => choices.push(resolve));
    if (command === 'online_blueprint_details') return { ...page, detail: (args as { id: string }).id === other.id ? other : item };
    return { ...page, items: [item, other] };
  });
  const wrapper = mount(OnlineBlueprints, { props });
  await openDetail(wrapper);
  await wrapper.findAll('button').find(button => button.attributes('aria-label') === other.title)!.trigger('click'); await flushPromises();
  choices[1]!({ ...singleChoices, viewer_id: 'new-viewer', viewers: [{ id: 'new-viewer', name: 'New Viewer', file_extensions: ['schem'] }] }); await flushPromises();
  choices[0]!({ ...singleChoices, viewer_id: 'old-viewer', viewers: [{ id: 'old-viewer', name: 'Old Viewer', file_extensions: ['schem'] }] }); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Preview')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_online_blueprint', expect.objectContaining({ id: other.id, viewerId: 'new-viewer' }));
  wrapper.unmount();
});

it('shows All versions only for an actual native search fallback and clears it with the old results', async () => {
  let fallback = true;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_blueprint_provider_availability') return { available: true, provider_id: 'community-provider', providers: [{ id: 'community-provider', name: 'Community', origins: [] }], viewer_available: true };
    return fallback ? { ...page, search_scope: 'all_versions' } : page;
  });
  const wrapper = mount(OnlineBlueprints, { props: { games: [{ ...game, version: '26.3' }], initialGameId: game.id } });
  await flushPromises();
  expect(wrapper.find('.online-search-scope').exists()).toBe(false);
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.get('.online-search-scope').text()).toBe('All versions');
  await wrapper.get('input[type="search"]').setValue('another search');
  expect(wrapper.find('.online-search-scope').exists()).toBe(false);
  fallback = false;
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.find('.online-search-scope').exists()).toBe(false);
  wrapper.unmount();
});
