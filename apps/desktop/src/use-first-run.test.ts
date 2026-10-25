// SPDX-License-Identifier: MIT
import { beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useFirstRun } from './use-first-run';
import { FIRST_RUN_CONSENT_VERSION } from './first-run';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset(); });
it('records Mesh opt-in only after the actual network setting succeeds', async () => {
  const calls: string[] = [];
  vi.mocked(invoke).mockImplementation(async command => { calls.push(command); return { meshChoice: true, consentVersion: FIRST_RUN_CONSENT_VERSION }; });
  const setup = useFirstRun();
  await setup.load(); calls.length = 0;
  expect(await setup.save('meshChoice', { meshChoice: true })).toBe(true);
  expect(calls).toEqual(['mesh_network_set_enabled', 'save_onboarding_state']);
});
it('does not record a choice when enabling Mesh fails', async () => {
  const setup = useFirstRun();
  vi.mocked(invoke).mockResolvedValueOnce({ consentVersion: FIRST_RUN_CONSENT_VERSION });
  await setup.load(); vi.mocked(invoke).mockClear();
  vi.mocked(invoke).mockRejectedValue(new Error('network unavailable'));
  expect(await setup.save('meshChoice', { meshChoice: true })).toBe(false);
  expect(invoke).toHaveBeenCalledTimes(1);
  expect(setup.state.value?.meshChoice).toBeUndefined();
  expect(setup.error.value).not.toContain('network unavailable');
  expect(setup.error.value).toContain('重试');
});
it('never enables network before consent, even if called outside the page', async () => {
  const setup = useFirstRun();
  expect(await setup.save('meshChoice', { meshChoice: true })).toBe(false);
  expect(invoke).not.toHaveBeenCalled();
  expect(setup.error.value).toContain('条款');
});
it('does not infer consent from a deferred flow', async () => {
  vi.mocked(invoke).mockResolvedValue({ deferred: true, consentVersion: null });
  const setup = useFirstRun();
  await setup.save('defer');
  expect(setup.consentAccepted()).toBe(false);
  expect(invoke).toHaveBeenCalledWith('save_onboarding_state', { action: 'defer' });
});
