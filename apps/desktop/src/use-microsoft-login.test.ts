// SPDX-License-Identifier: MIT
import { defineComponent } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useMicrosoftLogin, type MicrosoftLoginOptions } from './use-microsoft-login';
import { setLocale } from './i18n';
import type { MicrosoftLoginFlow, MicrosoftLoginPoll, ProfileLibrary } from './games';
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
const flow: MicrosoftLoginFlow = { flow_id: 'fixture-flow', user_code: 'FIXTURE1', verification_uri: 'https://example.invalid/device', expires_in: 30, interval: 1 };
const library: ProfileLibrary = { profiles: [{ id: 'saved-profile', name: 'Player', uuid: 'fixture-uuid', kind: 'microsoft', note: null }], active_id: 'saved-profile' };
const wrappers: ReturnType<typeof mount>[] = [];
function consumer(options: Partial<MicrosoftLoginOptions> = {}) {
  let login!: ReturnType<typeof useMicrosoftLogin>;
  const onComplete = vi.fn(), onAuthBusy = vi.fn();
  const wrapper = mount(defineComponent({ setup() { login = useMicrosoftLogin({ busy: () => false, onComplete, onAuthBusy, ...options }); return () => null; } }));
  wrappers.push(wrapper); return { login, wrapper, onComplete, onAuthBusy };
}
beforeEach(() => {
  vi.useFakeTimers(); setLocale('en-US'); vi.mocked(writeText).mockReset().mockResolvedValue();
  vi.mocked(invoke).mockReset().mockImplementation(async command => command === 'begin_microsoft_login' ? flow : command === 'poll_microsoft_login' ? { state: 'pending' } : null);
});
afterEach(async () => { for (const wrapper of wrappers.splice(0)) wrapper.unmount(); await flushPromises(); vi.useRealTimers(); setLocale('zh-CN'); });

it('completes once, clears in-flight flags and stops polling even if browser opening returns late', async () => {
  let opened!: () => void;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'begin_microsoft_login') return flow;
    if (command === 'open_microsoft_login') return new Promise<void>(resolve => { opened = resolve; });
    if (command === 'poll_microsoft_login') return { state: 'complete', library };
    return null;
  });
  const { login, onComplete } = consumer();
  const beginning = login.beginLogin(); await flushPromises();
  expect(login.loginStarting.value).toBe(true); expect(login.browserOpening.value).toBe(true);
  await vi.advanceTimersByTimeAsync(1000);
  expect(onComplete).toHaveBeenCalledExactlyOnceWith(library);
  expect(login.flow.value).toBeNull(); expect(login.loginStarting.value).toBe(false); expect(login.browserOpening.value).toBe(false);
  opened(); await beginning; await vi.advanceTimersByTimeAsync(60000);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'poll_microsoft_login')).toHaveLength(1);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'cancel_microsoft_login')).toBe(false);
});

it('cancels a late begin response without exposing a code or opening a browser', async () => {
  let started!: (flow: MicrosoftLoginFlow) => void;
  vi.mocked(invoke).mockImplementation(async command => command === 'begin_microsoft_login' ? new Promise<MicrosoftLoginFlow>(resolve => { started = resolve; }) : null);
  const { login, onComplete } = consumer();
  const beginning = login.beginLogin(); await flushPromises();
  expect(await login.cancelLogin()).toBe(true);
  started(flow); await beginning;
  expect(invoke).toHaveBeenCalledWith('cancel_microsoft_login', { flowId: flow.flow_id });
  expect(writeText).not.toHaveBeenCalled(); expect(onComplete).not.toHaveBeenCalled(); expect(login.flow.value).toBeNull();
});

it('expires through the host cancellation path and reports balanced cancellation busy state', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'begin_microsoft_login' ? { ...flow, expires_in: 2, interval: 5 } : null);
  const { login, onAuthBusy, onComplete } = consumer(); await login.beginLogin();
  await vi.advanceTimersByTimeAsync(2000);
  expect(invoke).toHaveBeenCalledWith('cancel_microsoft_login', { flowId: flow.flow_id });
  expect(login.flow.value).toBeNull(); expect(login.loginError.value).toContain('expired');
  expect(onAuthBusy.mock.calls).toEqual([[true], [false]]); expect(onComplete).not.toHaveBeenCalled();
});

it('retains the active flow when host cancellation fails and allows a retry', async () => {
  const { login, onAuthBusy } = consumer(); await login.beginLogin();
  vi.mocked(invoke).mockRejectedValueOnce(Error('cancel failed'));
  expect(await login.cancelLogin()).toBe(false); expect(login.flow.value?.flow_id).toBe(flow.flow_id);
  expect(login.loginError.value).not.toBe(''); expect(login.loginCancelling.value).toBe(false);
  expect(await login.cancelLogin()).toBe(true); expect(login.flow.value).toBeNull();
  expect(onAuthBusy.mock.calls).toEqual([[true], [false], [true], [false]]);
});

it('shows a local recovery action when the credential store is unavailable', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'begin_microsoft_login') return flow;
    if (command === 'poll_microsoft_login') throw 'AUTH_VAULT_UNAVAILABLE';
    return null;
  });
  const { login } = consumer(); await login.beginLogin(); await vi.advanceTimersByTimeAsync(1000);
  expect(login.flow.value).toBeNull();
  expect(login.loginError.value).toBe('Unlock or start your system credential store, then retry sign-in.');
  setLocale('zh-CN');
  await login.beginLogin(); await vi.advanceTimersByTimeAsync(1000);
  expect(login.loginError.value).toBe('请解锁或启动系统凭证库，然后重试登录。');
});

it('ignores late polling completion after its consumer unmounts', async () => {
  let finishPoll!: (value: MicrosoftLoginPoll) => void;
  vi.mocked(invoke).mockImplementation(async command => command === 'begin_microsoft_login' ? flow : command === 'poll_microsoft_login' ? new Promise<MicrosoftLoginPoll>(resolve => { finishPoll = resolve; }) : null);
  const { login, wrapper, onComplete } = consumer(); await login.beginLogin(); await vi.advanceTimersByTimeAsync(1000);
  wrapper.unmount(); await flushPromises(); finishPoll({ state: 'complete', library }); await flushPromises();
  expect(onComplete).not.toHaveBeenCalled(); expect(invoke).toHaveBeenCalledWith('cancel_microsoft_login', { flowId: flow.flow_id });
  expect(vi.getTimerCount()).toBe(0);
});

it('creates no late clipboard status timer after unmount during a failed copy', async () => {
  let failCopy!: (cause: Error) => void;
  vi.mocked(writeText).mockImplementation(() => new Promise<void>((_resolve, reject) => { failCopy = reject; }));
  const { login, wrapper } = consumer(); const beginning = login.beginLogin(); await flushPromises();
  wrapper.unmount(); await flushPromises(); failCopy(Error('clipboard unavailable')); await beginning;
  expect(vi.getTimerCount()).toBe(0); expect(login.copyStatus.value).toBe('');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'open_microsoft_login')).toBe(false);
});
