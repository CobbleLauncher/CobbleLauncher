<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import ValidatedTextField from './ui/ValidatedTextField.vue';
import { profileNameError } from './profile-name';
import { useMicrosoftLogin } from './use-microsoft-login';
import type { ProfileLibrary } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; error: string; ready: boolean }>();
const emit = defineEmits<{ add: [name: string, note: string | null]; select: [id: string]; cancel: []; libraryUpdated: [library: ProfileLibrary]; authBusy: [value: boolean] }>();
const mode = ref<'offline' | 'microsoft'>('offline');
const name = ref('');
const note = ref('');
const nameField = ref<InstanceType<typeof ValidatedTextField>>();
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
  if (props.busy || !props.ready || !nameField.value?.validate()) return;
  if (duplicate.value) emit('select', duplicate.value.id);
  else emit('add', name.value, note.value.trim() || null);
}
const { flow, loginStarting, loginCancelling, browserOpening, loginError, copyStatus, beginLogin: startLogin, cancelLogin, copyCode, openBrowser } = useMicrosoftLogin({
  busy: () => props.busy,
  onComplete: library => emit('libraryUpdated', library),
  onAuthBusy: value => emit('authBusy', value),
});
async function beginLogin() {
  if (props.busy || loginStarting.value || loginCancelling.value) return;
  mode.value = 'microsoft'; await startLogin();
}
async function chooseOffline() { if (await cancelLogin()) { mode.value = 'offline'; loginError.value = ''; } }
defineExpose({ cancelLogin });
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
      <form
        class="create-form"
        novalidate
        @submit.prevent="submit"
      >
        <ValidatedTextField
          id="profile-name"
          ref="nameField"
          v-model="name"
          :label="t('profiles.gameName')"
          :hint="t('profiles.nameHelp')"
          :validator="profileNameError"
          :limit="16"
          :placeholder="t('profiles.namePlaceholder')"
          :disabled="busy"
        />
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
          <summary><AppIcon name="chevron-right" />{{ t('profiles.offlineExplanationTitle') }}</summary><p>{{ t('profiles.offlineExplanationIdentity') }}</p><p>{{ t('profiles.offlineExplanationSwitch') }}</p>
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
<style scoped>
.profile-add-page{max-width:680px}
.profile-add-page{font-size:var(--ui-text-body,14px);line-height:calc(var(--font-size-base,14px)*22/14)}.profile-add-page .profile-kind-option,.profile-add-page .profile-intro h2,.profile-add-page .microsoft-login h2,.profile-add-page .profile-form-actions button,.profile-add-page .form-field label,.profile-add-page .form-field input{font-size:var(--ui-text-body,14px);line-height:calc(var(--font-size-base,14px)*22/14)}.profile-add-page .setting-hint,.profile-add-page .field-note,.profile-add-page .profile-explanation,.profile-add-page .microsoft-code-help{font-size:var(--ui-text-caption,12px);line-height:calc(var(--font-size-base,14px)*18/14)}.profile-add-page .microsoft-user-code{font-size:var(--ui-text-body,14px);line-height:calc(var(--font-size-base,14px)*22/14);letter-spacing:2px}.profile-add-page .profile-intro{gap:14px;margin-bottom:24px}.profile-add-page .profile-kind-option[aria-pressed=true]{border-color:var(--ui-accent,var(--ui-focus))}.profile-add-page .profile-kind-options{margin-bottom:20px}.profile-add-page .microsoft-login{padding:20px}
.profile-explanation summary{display:flex;align-items:center;gap:var(--ui-space-2);list-style:none;cursor:pointer}.profile-explanation summary::-webkit-details-marker{display:none}.profile-explanation summary::marker{content:''}.profile-explanation summary :deep(svg){width:14px;height:14px;flex:none;transition:transform .15s ease}.profile-explanation[open] summary :deep(svg){transform:rotate(90deg)}
</style>
