// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises } from '@vue/test-utils';
import { nextTick } from 'vue';
import ProfilesPage from './ProfilesPage.vue';
import { setLocale } from './i18n';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { avatars } from './appearance';
import type { PlayerProfile } from './games';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
vi.mock('./ProfileSkinPanel.vue', () => ({ default: { props: ['profile'], template: '<div class="mock-skin-panel">{{ profile.name }}</div>' } }));

const offline = { id: 'offline-1', kind: 'offline' as const, name: 'Luna', uuid: 'offline-uuid', note: null };
const microsoft = { id: 'microsoft-1', kind: 'microsoft' as const, name: 'Player', uuid: 'player-uuid', note: null };

beforeEach(() => { vi.mocked(invoke).mockReset(); vi.mocked(writeText).mockReset().mockResolvedValue(undefined); avatars.clear(); });
afterEach(() => { setLocale('zh-CN'); avatars.clear(); document.body.innerHTML = ''; });

function mountProfiles(profiles: PlayerProfile[] = [offline]) {
  return mount(ProfilesPage, {
    attachTo: document.body,
    props: {
      library: { profiles, active_id: null },
      busy: false,
      error: '',
      ready: true,
      choosing: false,
    },
  });
}

async function openMenu(profileName: string) {
  const trigger = document.body.querySelector(`button[aria-label="Manage ${profileName}"]`);
  if (!trigger) throw new Error(`Missing menu trigger for ${profileName}`);
  trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  await flushPromises();
}

async function chooseMenuItem(label: string) {
  const item = [...document.body.querySelectorAll('[role="menuitem"]')].find((candidate) => candidate.textContent?.includes(label));
  if (!item) throw new Error(`Missing menu item: ${label}`);
  item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  await flushPromises();
}

it('renders profile names and actions in English after switching languages', async () => {
  const wrapper = mountProfiles();

  setLocale('en-US');
  await nextTick();

  expect(wrapper.find('[aria-label="Player profiles"]').exists()).toBe(true);
  expect(wrapper.text()).toContain('Luna');
  expect(wrapper.text()).toContain('Offline');
  expect(wrapper.get('button[aria-label="Use Luna"]').text()).toBe('Use');
  expect(wrapper.findAll('button').some((button) => button.text().includes('Add profile'))).toBe(true);
  wrapper.unmount();
});

it('shows a cached skin avatar in the profile list before opening skin controls', () => {
  avatars.set(offline.id, 'data:image/png;base64,cached-avatar');
  const wrapper = mountProfiles();

  expect(wrapper.get('.profile-row-main img').attributes('src')).toBe('data:image/png;base64,cached-avatar');
  expect(wrapper.find('.mock-skin-panel').exists()).toBe(false);
  wrapper.unmount();
});

it('copies a profile UUID and reports clipboard failure', async () => {
  setLocale('en-US');
  const wrapper = mountProfiles();
  await openMenu('Luna');
  await chooseMenuItem('Copy UUID');
  expect(writeText).toHaveBeenCalledWith(offline.uuid);
  expect(document.body.querySelector('[role="status"]')?.textContent).toContain('UUID copied');

  vi.mocked(writeText).mockRejectedValueOnce(new Error('clipboard unavailable'));
  await openMenu('Luna');
  await chooseMenuItem('Copy UUID');
  expect(document.body.querySelector('[role="alert"]')?.textContent).toContain('Could not copy the UUID');
  wrapper.unmount();
});

it('refreshes Microsoft appearance with loading and success feedback', async () => {
  setLocale('en-US');
  let finishRefresh!: (value: { skin: { avatar: string }; localOnly: boolean }) => void;
  vi.mocked(invoke).mockImplementation(() => new Promise((resolve) => { finishRefresh = resolve; }));
  const wrapper = mountProfiles([microsoft]);
  await openMenu('Player');
  await chooseMenuItem('Refresh appearance');

  expect(invoke).toHaveBeenCalledWith('get_profile_appearance', { id: microsoft.id, refresh: true });
  expect(document.body.querySelector('[role="status"]')?.textContent).toContain('Refreshing appearance');
  finishRefresh({ skin: { avatar: 'data:image/png;base64,updated-avatar' }, localOnly: false });
  await flushPromises();

  expect(avatars.get(microsoft.id)).toBe('data:image/png;base64,updated-avatar');
  expect(document.body.querySelector('[role="status"]')?.textContent).toContain('Appearance refreshed');
  wrapper.unmount();
});

it('reports appearance refresh failure without changing the cached avatar', async () => {
  setLocale('en-US');
  avatars.set(microsoft.id, 'data:image/png;base64,old-avatar');
  vi.mocked(invoke).mockRejectedValueOnce(new Error('offline'));
  const wrapper = mountProfiles([microsoft]);
  await openMenu('Player');
  await chooseMenuItem('Refresh appearance');
  expect(await document.body.querySelector('[role="alert"]')?.textContent).toContain('Could not refresh profile appearance');
  expect(avatars.get(microsoft.id)).toBe('data:image/png;base64,old-avatar');
  wrapper.unmount();
});

it('keeps skin, note, and removal actions available', async () => {
  setLocale('en-US');
  const wrapper = mountProfiles();
  await openMenu('Luna');
  await chooseMenuItem('View and set skin');
  expect(wrapper.find('.mock-skin-panel').exists()).toBe(true);

  await openMenu('Luna');
  await chooseMenuItem('Edit note');
  expect(wrapper.find('input#note-offline-1').exists()).toBe(true);

  await openMenu('Luna');
  await chooseMenuItem('Remove profile');
  expect(wrapper.text()).toContain('Remove profile Luna? Games and saves will be kept.');
  await wrapper.get('.profile-remove-confirm .secondary-button').trigger('click');
  expect(wrapper.emitted('remove')?.[0]).toEqual([offline.id]);
  wrapper.unmount();
});
