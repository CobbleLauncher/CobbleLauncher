<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { t } from './i18n';
import type { MicrosoftLoginFlow, MicrosoftLoginPoll, ProfileLibrary } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; error: string; ready: boolean }>();
const emit = defineEmits<{ add: [name: string, note: string | null]; select: [id: string]; cancel: []; libraryUpdated: [library: ProfileLibrary]; authBusy: [value: boolean] }>();
const mode = ref<'offline' | 'microsoft'>('offline');
const name = ref('');
const note = ref('');
const attempted = ref(false);
const valid = computed(() => /^[A-Za-z0-9_]{3,16}$/.test(name.value));
const duplicate = computed(() => props.library.profiles.find((profile) => profile.kind === 'offline' && profile.name === name.value));
function profileError(error: string) {
  const known: Record<string, string> = {
    '无法读取玩家档案，请重试': t('profiles.errors.read'),
    '档案已保存，请重新选择游戏使用的档案。': t('profiles.errors.saved'),
    '请在桌面应用中保存玩家档案': t('profiles.errors.desktopSave'),
    '未能保存档案，请重试': t('profiles.errors.save'),
  };
  return known[error] ?? error;
}
function submit() {
  attempted.value = true;
  if (!valid.value || props.busy || !props.ready) return;
  if (duplicate.value) emit('select', duplicate.value.id);
  else emit('add', name.value, note.value.trim() || null);
}
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
  loginCancelling.value = true; emit('authBusy', true);
  try { await invoke('cancel_microsoft_login', { flowId: active.flow_id }); flow.value = null; return true; }
  catch { loginError.value = t('profiles.loginErrors.cancel'); return false; }
  finally { loginCancelling.value = false; emit('authBusy', false); }
}
async function chooseOffline() { if (await cancelLogin()) { mode.value = 'offline'; loginError.value = ''; } }
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
      stopTimers(); flow.value = null; ++generation;
      emit('libraryUpdated', value.library);
    } else schedulePoll(current);
  } catch (cause) {
    if (current !== generation || disposed) return;
    if (await cancelLogin()) loginError.value = t('profiles.loginErrors.poll', { error: String(cause) });
  }
}
async function beginLogin() {
  if (props.busy || loginStarting.value || loginCancelling.value) return;
  mode.value = 'microsoft'; loginError.value = '';
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
defineExpose({ cancelLogin });
onUnmounted(() => { disposed = true; clearTimeout(copyTimer); void cancelLogin(); });
</script>
<template>
  <div class="page-content profile-add-page">
    <div
      class="profile-kind-options"
      role="group"
      :aria-label="t('profiles.kindLabel')"
    >
      <button
        class="profile-kind-option"
        :aria-pressed="mode === 'offline'"
        :disabled="busy || loginCancelling"
        @click="chooseOffline"
      >
        {{ t('profiles.offline') }}
      </button>
      <button
        class="profile-kind-option"
        :aria-pressed="mode === 'microsoft'"
        :disabled="busy || loginStarting || loginCancelling || !!flow"
        @click="beginLogin"
      >
        {{ t('profiles.microsoft') }}
      </button>
    </div>
    <section
      v-if="mode === 'microsoft'"
      class="microsoft-login"
      :aria-label="t('profiles.microsoftLabel')"
    >
      <h2>{{ t('profiles.signInTitle') }}</h2>
      <p class="setting-hint">
        {{ t('profiles.signInHint') }}
      </p>
      <p
        v-if="loginStarting && !flow"
        class="setting-hint"
        role="status"
      >
        {{ t('profiles.preparing') }}
      </p>
      <button
        v-if="loginStarting && !flow"
        class="text-button"
        @click="chooseOffline"
      >
        {{ t('profiles.cancelLogin') }}
      </button>
      <template v-if="flow">
        <p class="microsoft-code-help">
          {{ t('profiles.enterCode') }}
        </p>
        <button
          type="button"
          class="microsoft-user-code"
          :aria-label="t('profiles.copyCode')"
          @click="copyCode"
        >
          {{ flow.user_code }}
        </button>
        <span
          class="setting-hint"
          role="status"
        >{{ copyStatus }}</span>
        <p
          v-if="!loginError"
          class="setting-hint"
          role="status"
        >
          {{ t('profiles.waiting') }}
        </p>
        <div class="profile-form-actions">
          <button
            class="secondary-button"
            :disabled="browserOpening || loginCancelling"
            @click="openBrowser"
          >
            {{ t('profiles.reopenBrowser') }}
          </button><button
            class="text-button"
            :disabled="loginCancelling"
            @click="chooseOffline"
          >
            {{ loginCancelling ? t('profiles.cancelling') : t('profiles.cancelLogin') }}
          </button>
        </div>
      </template>
      <div
        v-else-if="!loginStarting"
        class="profile-form-actions"
      >
        <button
          class="primary-button"
          :disabled="busy || loginCancelling"
          @click="beginLogin"
        >
          {{ t('profiles.retryLogin') }}
        </button><button
          class="text-button"
          :disabled="loginCancelling"
          @click="chooseOffline"
        >
          {{ t('profiles.backToOffline') }}
        </button>
      </div>
      <p
        v-if="loginError"
        class="settings-error"
        role="alert"
      >
        {{ loginError }}
      </p>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ profileError(error) }}
      </p>
    </section>
    <template v-else>
      <div class="profile-intro">
        <span
          class="profile-avatar large"
          aria-hidden="true"
        >{{ name.slice(0, 1).toUpperCase() || '+' }}</span><div><h2>{{ t('profiles.offlineTitle') }}</h2></div>
      </div>
      <form
        class="create-form"
        @submit.prevent="submit"
      >
        <div class="form-field">
          <label for="profile-name">{{ t('profiles.gameName') }}</label><input
            id="profile-name"
            v-model="name"
            autocomplete="off"
            spellcheck="false"
            maxlength="16"
            :placeholder="t('profiles.namePlaceholder')"
            :disabled="busy"
            :aria-invalid="attempted && !valid"
            aria-describedby="profile-name-help"
          ><p
            id="profile-name-help"
            class="setting-hint"
          >
            {{ t('profiles.nameHelp') }}
          </p><p
            v-if="attempted && !valid"
            class="settings-error"
            role="alert"
          >
            {{ t('profiles.nameInvalid') }}
          </p>
        </div>
        <p
          v-if="duplicate"
          class="profile-notice"
        >
          {{ t('profiles.duplicate') }}
        </p>
        <div
          v-else
          class="form-field"
        >
          <label for="profile-note">{{ t('profiles.note') }} <span class="field-note">{{ t('profiles.optional') }}</span></label><input
            id="profile-note"
            v-model="note"
            maxlength="160"
            :placeholder="t('profiles.notePlaceholder')"
            :disabled="busy"
          >
        </div>
        <details class="profile-explanation">
          <summary>{{ t('profiles.offlineExplanationTitle') }}</summary><p>{{ t('profiles.offlineExplanationIdentity') }}</p><p>{{ t('profiles.offlineExplanationSwitch') }}</p>
        </details>
        <p
          v-if="error"
          class="settings-error"
          role="alert"
        >
        {{ profileError(error) }}
        </p>
        <div class="profile-form-actions">
          <button
            class="primary-button"
            type="submit"
            :disabled="busy || !ready"
          >
            {{ busy ? t('profiles.saving') : duplicate ? t('profiles.useProfile') : t('profiles.addAndUse') }}
          </button><button
            class="text-button"
            type="button"
            :disabled="busy"
            @click="$emit('cancel')"
          >
            {{ t('profiles.cancel') }}
          </button>
        </div>
      </form>
    </template>
  </div>
</template>
