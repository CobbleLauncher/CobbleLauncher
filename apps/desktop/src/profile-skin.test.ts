// SPDX-License-Identifier: MIT
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import ProfileSkinPanel from './ProfileSkinPanel.vue';
const viewer = vi.hoisted(() => ({ loadSkin: vi.fn().mockResolvedValue(undefined), dispose: vi.fn(), controls: { enableZoom: true }, autoRotate: false }));
vi.mock('skinview3d', () => ({ SkinViewer: class { constructor() { return viewer; } } }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const profile = { id: 'owned-profile', kind: 'microsoft' as const, name: 'Player', uuid: 'owned', note: null };
const skin = { image: 'data:image/png;base64,synthetic', avatar: 'data:image/png;base64,avatar', model: 'classic' as const };
describe('profile skin workflow without WebGL', () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); viewer.loadSkin.mockClear(); viewer.dispose.mockClear(); });
  it('renders the host-provided appearance and disposes the viewer', async () => {
    vi.mocked(invoke).mockResolvedValue({ skin, localOnly: false });
    const wrapper = mount(ProfileSkinPanel, { props: { profile } });
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith('get_profile_appearance', { id: profile.id, refresh: false });
    expect(viewer.loadSkin).toHaveBeenCalledWith(skin.image, { model: 'default' });
    expect(wrapper.emitted('avatar')?.[0]).toEqual([skin.avatar]);
    wrapper.unmount();
    expect(viewer.dispose).toHaveBeenCalledOnce();
  });
  it('requires an explicit apply action and sends only profile id and model', async () => {
    vi.mocked(invoke).mockImplementation(async (command) => command === 'pick_profile_skin' ? skin : { skin: command === 'apply_profile_skin' ? skin : null, localOnly: false });
    const wrapper = mount(ProfileSkinPanel, { props: { profile } });
    await flushPromises();
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'apply_profile_skin')).toBe(false);
    await wrapper.get('select').setValue('slim');
    await wrapper.get('.primary-button').trigger('click');
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith('apply_profile_skin', { id: profile.id, model: 'slim' });
    expect(wrapper.find('select').exists()).toBe(false);
    wrapper.unmount();
  });
  it('keeps a failed application available for retry', async () => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'apply_profile_skin') throw new Error('account unavailable');
      return command === 'pick_profile_skin' ? skin : { skin: null, localOnly: false };
    });
    const wrapper = mount(ProfileSkinPanel, { props: { profile } });
    await flushPromises();
    await wrapper.get('button').trigger('click'); await flushPromises();
    await wrapper.get('.primary-button').trigger('click'); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('account unavailable');
    expect(wrapper.find('.primary-button').exists()).toBe(true);
    wrapper.unmount();
  });
  it('labels offline skins as launcher-only and leaves cancelled picks unchanged', async () => {
    vi.mocked(invoke).mockImplementation(async (command) => command === 'pick_profile_skin' ? null : { skin: null, localOnly: true });
    const wrapper = mount(ProfileSkinPanel, { props: { profile: { ...profile, kind: 'offline' } } });
    await flushPromises();
    expect(wrapper.text()).toContain('仅在启动器中展示');
    await wrapper.get('button').trigger('click'); await flushPromises();
    expect(wrapper.find('.primary-button').exists()).toBe(false);
    wrapper.unmount();
  });
});
