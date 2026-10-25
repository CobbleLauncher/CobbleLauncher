// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { setLocale } from '../i18n';
import EnvironmentInstallReview from './EnvironmentInstallReview.vue';
import type { EnvironmentInstallationPreview } from './types';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const props = { planId: 'reviewed-plan', source: { source: 'official' as const, mirror: '' }, useDefaultDirectory: true };
const preview: EnvironmentInstallationPreview = { loader_version: '0.16.14', requirements: { ready: true, missing: [] }, space: { can_proceed: true, exact: true, volumes: [{ volume: { id: 'test', label: 'Game drive', available_bytes: 20 * 1024 ** 3 }, additional_peak_bytes: 2 * 1024 ** 3, unknown_sizes: 0, verdict: 'sufficient' }] } };
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); });
afterEach(() => setLocale('zh-CN'));
it('waits for native readiness and reruns preflight when destination settings change', async () => {
  let resolve!: (value: EnvironmentInstallationPreview) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(yes => { resolve = yes; })).mockResolvedValue(preview);
  const wrapper = mount(EnvironmentInstallReview, { props });
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([false]);
  resolve(preview); await flushPromises();
  expect(wrapper.text()).toContain('2.0 GiB');
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([true]);
  await wrapper.setProps({ useDefaultDirectory: false }); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('preview_environment_installation', { planId: 'reviewed-plan', source: props.source, useDefaultDirectory: false });
  wrapper.unmount();
});
it('blocks insufficient disk space and presents the measured volume', async () => {
  vi.mocked(invoke).mockResolvedValue({ ...preview, space: { ...preview.space, can_proceed: false, volumes: [{ ...preview.space.volumes[0]!, verdict: 'insufficient' }] } });
  const wrapper = mount(EnvironmentInstallReview, { props }); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('not enough disk space');
  expect(wrapper.text()).toContain('Game drive');
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([false]); wrapper.unmount();
});
it('requires verified manual files, preserves the review on cancel, and passes exact provider identifiers', async () => {
  const missing = { project_id: '123', file_id: '456', name: 'Manual Mod', file_name: 'manual.jar', size_bytes: 1, website_url: 'https://www.curseforge.com/minecraft/mc-mods/manual', reason: 'manual' };
  vi.mocked(invoke).mockResolvedValueOnce({ ...preview, requirements: { ready: false, missing: [missing] } }).mockResolvedValueOnce(null).mockResolvedValueOnce({ ready: true, missing: [] });
  const wrapper = mount(EnvironmentInstallReview, { props }); await flushPromises();
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([false]);
  await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('manual.jar');
  await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('select_environment_manual_file', { planId: 'reviewed-plan', projectId: '123', fileId: '456' });
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([true]); wrapper.unmount();
});
