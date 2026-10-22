<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { MicrosoftLoginFlow, MicrosoftLoginPoll, ProfileLibrary } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; error: string; ready: boolean }>();
const emit = defineEmits<{ add: [name: string, note: string | null]; select: [id: string]; cancel: []; libraryUpdated: [library: ProfileLibrary]; authBusy: [value: boolean] }>();
const mode = ref<'offline' | 'microsoft'>('offline');
const name = ref('');
const note = ref('');
const attempted = ref(false);
const valid = computed(() => /^[A-Za-z0-9_]{3,16}$/.test(name.value));
const duplicate = computed(() => props.library.profiles.find((profile) => profile.kind === 'offline' && profile.name === name.value));
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
  catch { loginError.value = '未能取消登录，请重试。'; return false; }
  finally { loginCancelling.value = false; emit('authBusy', false); }
}
async function chooseOffline() { if (await cancelLogin()) { mode.value = 'offline'; loginError.value = ''; } }
async function openBrowser() {
  if (!flow.value || browserOpening.value || loginCancelling.value) return;
  const current = generation;
  const flowId = flow.value.flow_id;
  browserOpening.value = true;
  try { await invoke('open_microsoft_login', { flowId }); if (current === generation && !disposed) loginError.value = ''; }
  catch { if (current === generation && !disposed) loginError.value = '无法打开登录页面，请重新打开浏览器。'; }
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
      if (!value.library) throw new Error('登录结果不完整，请重试。');
      stopTimers(); flow.value = null; ++generation;
      emit('libraryUpdated', value.library);
    } else schedulePoll(current);
  } catch (cause) {
    if (current !== generation || disposed) return;
    if (await cancelLogin()) loginError.value = `登录未完成：${String(cause)}`;
  }
}
async function beginLogin() {
  if (props.busy || loginStarting.value || loginCancelling.value) return;
  mode.value = 'microsoft'; loginError.value = '';
  if (!isTauri()) { loginError.value = '请在桌面应用中登录 Microsoft 账号'; return; }
  if (!await cancelLogin()) return;
  const current = ++generation;
  loginStarting.value = true;
  try {
    const value = await invoke<MicrosoftLoginFlow>('begin_microsoft_login');
    if (current !== generation || disposed) { void invoke('cancel_microsoft_login', { flowId: value.flow_id }).catch(() => {}); return; }
    flow.value = value;
    expiryTimer = setTimeout(() => { if (current === generation) void cancelLogin().then((cancelled) => { if (cancelled && !disposed) loginError.value = '登录已过期，请重试。'; }); }, Math.max(0, value.expires_in) * 1000);
    schedulePoll(current);
    await openBrowser();
  } catch (cause) { if (current === generation && !disposed) loginError.value = `无法开始 Microsoft 登录：${String(cause)}`; }
  finally { if (current === generation) loginStarting.value = false; }
}
defineExpose({ cancelLogin });
onUnmounted(() => { disposed = true; void cancelLogin(); });
</script>
<template>
  <div class="page-content profile-add-page">
    <div
      class="profile-kind-options"
      role="group"
      aria-label="档案类型"
    >
      <button
        class="profile-kind-option"
        :aria-pressed="mode === 'offline'"
        :disabled="busy || loginCancelling"
        @click="chooseOffline"
      >
        离线档案
      </button>
      <button
        class="profile-kind-option"
        :aria-pressed="mode === 'microsoft'"
        :disabled="busy || loginStarting || loginCancelling || !!flow"
        @click="beginLogin"
      >
        Microsoft 登录
      </button>
    </div>
    <section
      v-if="mode === 'microsoft'"
      class="microsoft-login"
      aria-label="Microsoft 登录"
    >
      <h2>登录正版账号</h2>
      <p class="setting-hint">
        使用拥有 Minecraft Java 版的 Microsoft 账号。
      </p>
      <p
        v-if="loginStarting && !flow"
        class="setting-hint"
        role="status"
      >
        正在准备 Microsoft 登录…
      </p>
      <button
        v-if="loginStarting && !flow"
        class="text-button"
        @click="chooseOffline"
      >
        取消登录
      </button>
      <template v-if="flow">
        <p class="microsoft-code-help">
          在打开的 Microsoft 页面中输入代码，完成账号授权。
        </p>
        <code
          class="microsoft-user-code"
          aria-label="登录代码"
          tabindex="0"
        >{{ flow.user_code }}</code>
        <p
          v-if="!loginError"
          class="setting-hint"
          role="status"
        >
          等待浏览器授权…
        </p>
        <div class="profile-form-actions">
          <button
            class="secondary-button"
            :disabled="browserOpening || loginCancelling"
            @click="openBrowser"
          >
            重新打开浏览器
          </button><button
            class="text-button"
            :disabled="loginCancelling"
            @click="chooseOffline"
          >
            {{ loginCancelling ? '取消中…' : '取消登录' }}
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
          重试登录
        </button><button
          class="text-button"
          :disabled="loginCancelling"
          @click="chooseOffline"
        >
          返回离线添加
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
        {{ error }}
      </p>
    </section>
    <template v-else>
      <div class="profile-intro">
        <span
          class="profile-avatar large"
          aria-hidden="true"
        >{{ name.slice(0, 1).toUpperCase() || '+' }}</span><div><h2>离线档案</h2></div>
      </div>
      <form
        class="create-form"
        @submit.prevent="submit"
      >
        <div class="form-field">
          <label for="profile-name">游戏昵称</label><input
            id="profile-name"
            v-model="name"
            autocomplete="off"
            spellcheck="false"
            maxlength="16"
            placeholder="例如 Dylan"
            :disabled="busy"
            :aria-invalid="attempted && !valid"
            aria-describedby="profile-name-help"
          ><p
            id="profile-name-help"
            class="setting-hint"
          >
            3–16 位英文字母、数字或下划线，区分大小写。
          </p><p
            v-if="attempted && !valid"
            class="settings-error"
            role="alert"
          >
            昵称须为 3–16 位英文字母、数字或下划线
          </p>
        </div>
        <p
          v-if="duplicate"
          class="profile-notice"
        >
          这个昵称已在档案中。可以直接使用已有身份。
        </p>
        <div
          v-else
          class="form-field"
        >
          <label for="profile-note">备注 <span class="field-note">可选</span></label><input
            id="profile-note"
            v-model="note"
            maxlength="160"
            placeholder="例如 建筑存档使用"
            :disabled="busy"
          >
        </div>
        <details class="profile-explanation">
          <summary>离线身份和存档</summary><p>昵称决定玩家 UUID。不同昵称进入同一个世界时，背包、位置等玩家数据可能不同。昵称创建后不能修改；需要新昵称时，请添加另一个档案。</p><p>档案切换不会复制游戏或隔离存档。</p>
        </details>
        <p
          v-if="error"
          class="settings-error"
          role="alert"
        >
          {{ error }}
        </p>
        <div class="profile-form-actions">
          <button
            class="primary-button"
            type="submit"
            :disabled="busy || !ready"
          >
            {{ busy ? '保存中…' : duplicate ? '使用此档案' : '添加并使用' }}
          </button><button
            class="text-button"
            type="button"
            :disabled="busy"
            @click="$emit('cancel')"
          >
            取消
          </button>
        </div>
      </form>
    </template>
  </div>
</template>
