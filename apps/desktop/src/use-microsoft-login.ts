// SPDX-License-Identifier: MIT
import { onUnmounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { t } from './i18n';
import type { MicrosoftLoginFlow, MicrosoftLoginPoll, ProfileLibrary } from './games';

export interface MicrosoftLoginOptions {
  busy: () => boolean;
  onComplete: (library: ProfileLibrary) => void;
  onAuthBusy?: (busy: boolean) => void;
}

/** Shared device-login lifecycle. The consumer owns its screen and profile selection. */
export function useMicrosoftLogin(options: MicrosoftLoginOptions) {
  const flow = ref<MicrosoftLoginFlow | null>(null);
  const loginStarting = ref(false);
  const loginCancelling = ref(false);
  const browserOpening = ref(false);
  const loginError = ref('');
  const copyStatus = ref('');
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  async function copyCode() {
    const active = flow.value;
    if (!active) return;
    try {
      await writeText(active.user_code);
      if (flow.value !== active || disposed) return;
      copyStatus.value = t('profiles.copied');
    } catch { if (flow.value === active && !disposed) copyStatus.value = t('profiles.copyFailed'); }
    if (flow.value !== active || disposed) return;
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => { copyStatus.value = ''; }, 2500);
  }
  let generation = 0;
  let disposed = false;
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  let expiryTimer: ReturnType<typeof setTimeout> | undefined;
  function stopTimers() { clearTimeout(pollTimer); clearTimeout(expiryTimer); pollTimer = undefined; expiryTimer = undefined; }
  async function cancelLogin(): Promise<boolean> {
    ++generation; stopTimers(); loginStarting.value = false; browserOpening.value = false;
    const active = flow.value;
    if (!active) return true;
    if (loginCancelling.value) return false;
    loginCancelling.value = true; options.onAuthBusy?.(true);
    try { await invoke('cancel_microsoft_login', { flowId: active.flow_id }); flow.value = null; return true; }
    catch { loginError.value = t('profiles.loginErrors.cancel'); return false; }
    finally { loginCancelling.value = false; options.onAuthBusy?.(false); }
  }
  async function openBrowser() {
    if (!flow.value || browserOpening.value || loginCancelling.value) return;
    const current = generation;
    const flowId = flow.value.flow_id;
    browserOpening.value = true;
    try { await invoke('open_microsoft_login', { flowId }); if (current === generation && !disposed) loginError.value = ''; }
    catch { if (current === generation && !disposed) loginError.value = t('profiles.loginErrors.browser'); }
    finally { if (current === generation) browserOpening.value = false; }
  }
  function schedulePoll(current: number) {
    if (current !== generation || !flow.value || disposed) return;
    pollTimer = setTimeout(() => { void poll(current); }, Math.max(1, flow.value.interval) * 1000);
  }
  async function poll(current: number) {
    const active = flow.value;
    if (!active || current !== generation || disposed) return;
    try {
      const value = await invoke<MicrosoftLoginPoll>('poll_microsoft_login', { flowId: active.flow_id });
      if (current !== generation || disposed) return;
      if (value.state === 'complete') {
        if (!value.library) throw new Error(t('profiles.loginErrors.incomplete'));
        stopTimers(); flow.value = null; loginStarting.value = false; browserOpening.value = false; ++generation;
        options.onComplete(value.library);
      } else schedulePoll(current);
    } catch (cause) {
      if (current !== generation || disposed) return;
      if (await cancelLogin()) loginError.value = t('profiles.loginErrors.poll', { error: String(cause) });
    }
  }
  async function beginLogin() {
    if (options.busy() || loginStarting.value || loginCancelling.value) return;
    loginError.value = '';
    if (!isTauri()) { loginError.value = t('profiles.loginErrors.notDesktop'); return; }
    if (!await cancelLogin()) return;
    const current = ++generation;
    loginStarting.value = true;
    try {
      const value = await invoke<MicrosoftLoginFlow>('begin_microsoft_login');
      if (current !== generation || disposed) { void invoke('cancel_microsoft_login', { flowId: value.flow_id }).catch(() => {}); return; }
      flow.value = value;
      expiryTimer = setTimeout(() => { if (current === generation) void cancelLogin().then((cancelled) => { if (cancelled && !disposed) loginError.value = t('profiles.loginErrors.expired'); }); }, Math.max(0, value.expires_in) * 1000);
      schedulePoll(current);
      await copyCode();
      if (current !== generation || disposed) return;
      await openBrowser();
    } catch (cause) { if (current === generation && !disposed) loginError.value = t('profiles.loginErrors.start', { error: String(cause) }); }
    finally { if (current === generation) loginStarting.value = false; }
  }
  onUnmounted(() => { disposed = true; clearTimeout(copyTimer); void cancelLogin(); });
  return { flow, loginStarting, loginCancelling, browserOpening, loginError, copyStatus, beginLogin, cancelLogin, copyCode, openBrowser };
}
