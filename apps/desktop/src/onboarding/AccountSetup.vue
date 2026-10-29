<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { ArrowLeft, ArrowRight, Copy, ExternalLink, Check, UserRound } from '@lucide/vue';
import { locale, t } from '../i18n';
import type { PlayerProfile, ProfileLibrary } from '../games';
import { useMicrosoftLogin } from '../use-microsoft-login';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
import ValidatedTextField from '../ui/ValidatedTextField.vue';
import { profileNameError } from '../profile-name';
import AccountArtwork from './AccountArtwork.vue';
import './account-setup.css';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; ready: boolean; error: string }>();
const emit = defineEmits<{ add: [name: string, note: string | null]; select: [id: string]; libraryUpdated: [library: ProfileLibrary]; authBusy: [busy: boolean]; back: []; skip: []; complete: [] }>();
const scene = ref<'choose' | 'microsoft' | 'offline' | 'complete'>('choose');
const zh = computed(() => locale.value === 'zh-CN');
const name = ref('');
const nameField = ref<InstanceType<typeof ValidatedTextField>>();
const pendingName = ref<string | null>(null);
const completed = ref<PlayerProfile | null>(null);
const duplicate = computed(() => props.library.profiles.find(profile => profile.kind === 'offline' && profile.name === name.value));
function finish(library: ProfileLibrary) {
  const profile = library.profiles.find(item => item.id === library.active_id);
  if (profile) { completed.value = profile; scene.value = 'complete'; }
}
const { flow, loginStarting, loginCancelling, browserOpening, loginError, copyStatus, beginLogin, cancelLogin, copyCode, openBrowser } = useMicrosoftLogin({
  busy: () => props.busy,
  onComplete: library => { finish(library); emit('libraryUpdated', library); },
  onAuthBusy: value => emit('authBusy', value),
});
watch(() => props.library, library => {
  if (pendingName.value && library.profiles.some(profile => profile.id === library.active_id && profile.name === pendingName.value && profile.kind === 'offline')) { pendingName.value = null; finish(library); }
});
function submit() {
  if (props.busy || !props.ready || !nameField.value?.validate()) return;
  pendingName.value = name.value;
  if (duplicate.value) emit('select', duplicate.value.id); else emit('add', name.value, null);
}
async function back() {
  if (props.busy || loginCancelling.value) return;
  if (scene.value === 'choose') emit('back');
  else if (await cancelLogin()) { scene.value = 'choose'; pendingName.value = null; }
}
function focusScene(element: Element) {
  (element.querySelector('[data-scene-focus]') as HTMLElement | null)?.focus({ preventScroll: true });
  element.parentElement?.scrollTo({ top: 0 });
}
defineExpose({ cancelLogin });
</script>
<template>
  <section
    class="onboarding-page account-setup"
    :aria-busy="busy || loginStarting || loginCancelling"
  >
    <div class="account-progress">
      <UserRound aria-hidden="true" /><span>{{ zh ? '玩家档案' : 'Player profile' }}</span><span class="guide-count">2 / 4</span>
    </div>
    <div class="account-stage">
      <Transition
        name="setup-step"
        mode="out-in"
        @after-enter="focusScene"
      >
        <div
          v-if="scene === 'choose'"
          key="choose"
          class="account-choice-scene"
        >
          <header>
            <h1
              data-scene-focus
              tabindex="-1"
            >
              {{ zh ? '用哪种身份开始？' : 'How would you like to play?' }}
            </h1><p>{{ zh ? '以后可以添加更多档案，随时切换。' : 'You can add more profiles and switch later.' }}</p>
          </header>
          <div class="account-choice-grid">
            <button
              class="account-choice"
              :disabled="busy || !ready"
              :aria-label="zh ? '正版账号' : 'Microsoft account'"
              @click="scene = 'microsoft'"
            >
              <AccountArtwork kind="microsoft" /><strong>{{ zh ? '正版账号' : 'Microsoft account' }}</strong><span>{{ zh ? '登录拥有 Minecraft 的账号' : 'Sign in with an account that owns Minecraft' }}</span><ArrowRight
                class="choice-arrow"
                aria-hidden="true"
              />
            </button>
            <button
              class="account-choice"
              :disabled="busy || !ready"
              :aria-label="zh ? '离线档案' : 'Offline profile'"
              @click="scene = 'offline'"
            >
              <AccountArtwork kind="offline" /><strong>{{ zh ? '离线档案' : 'Offline profile' }}</strong><span>{{ zh ? '取个名字，在本机开始' : 'Choose a name and play locally' }}</span><ArrowRight
                class="choice-arrow"
                aria-hidden="true"
              />
            </button>
          </div>
        </div>
        <div
          v-else-if="scene === 'offline'"
          key="offline"
          class="account-focus-scene"
        >
          <div class="account-illustration">
            <AccountArtwork kind="offline" />
          </div>
          <form
            id="guided-offline-profile"
            class="account-scene-content"
            novalidate
            @submit.prevent="submit"
          >
            <h1>{{ zh ? '给自己取个名字' : 'Choose your player name' }}</h1>
            <p>{{ zh ? '它会显示在游戏里。' : 'This is the name other players see.' }}</p>
            <ValidatedTextField
              id="guided-player-name"
              ref="nameField"
              v-model="name"
              data-scene-focus
              :label="t('profiles.gameName')"
              :hint="t('profiles.nameHelp')"
              :validator="profileNameError"
              :limit="16"
              placeholder="Alex"
              :disabled="busy"
            />
            <p
              v-if="duplicate"
              class="account-note"
            >
              {{ zh ? '这个档案已存在，将使用已有档案。' : 'This profile already exists. We will use it.' }}
            </p>
            <p class="account-note">
              {{ zh ? '离线档案无法加入要求正版验证的服务器。' : 'Offline profiles cannot join authenticated servers.' }}
            </p>
            <p
              v-if="error"
              class="settings-error"
              role="alert"
            >
              {{ error }}
            </p>
          </form>
        </div>
        <div
          v-else-if="scene === 'microsoft'"
          key="microsoft"
          class="account-focus-scene"
        >
          <div class="account-illustration">
            <AccountArtwork kind="microsoft" />
          </div>
          <div class="account-scene-content">
            <h1
              data-scene-focus
              tabindex="-1"
            >
              {{ flow ? (zh ? '在浏览器中完成登录' : 'Finish signing in in your browser') : (zh ? '连接你的 Microsoft 账号' : 'Connect your Microsoft account') }}
            </h1>
            <p>{{ flow ? (zh ? '输入下面的代码，完成后回到这里。' : 'Enter this code, then come back here.') : (zh ? '将在浏览器中打开安全登录页面。' : 'A secure sign-in page will open in your browser.') }}</p>
            <template v-if="flow">
              <button
                class="account-login-code"
                :aria-label="t('profiles.copyCode')"
                @click="copyCode"
              >
                {{ flow.user_code }}<Copy aria-hidden="true" />
              </button>
              <span
                class="account-copy-status"
                role="status"
              >{{ copyStatus }}</span>
              <LoadingIndicator
                compact
                :label="t('profiles.waiting')"
              />
            </template>
            <LoadingIndicator
              v-else-if="loginStarting"
              :label="t('profiles.preparing')"
            />
            <p
              v-if="loginError"
              class="settings-error"
              role="alert"
            >
              {{ loginError }}
            </p>
          </div>
        </div>
        <div
          v-else
          key="complete"
          class="account-complete-scene"
        >
          <div class="account-illustration">
            <AccountArtwork kind="complete" />
          </div>
          <h1
            data-scene-focus
            tabindex="-1"
          >
            {{ zh ? `你好，${completed?.name ?? ''}` : `Welcome, ${completed?.name ?? ''}` }}
          </h1>
          <p><Check aria-hidden="true" />{{ zh ? '档案已保存，接下来准备游戏。' : 'Your profile is saved. Let’s set up a game.' }}</p>
        </div>
      </Transition>
    </div>
    <footer class="onboarding-footer account-footer">
      <button
        v-if="scene !== 'complete'"
        class="secondary-button account-back"
        :aria-label="zh ? '上一步' : 'Back'"
        :disabled="busy || loginCancelling"
        @click="back"
      >
        <ArrowLeft aria-hidden="true" /><span>{{ zh ? '上一步' : 'Back' }}</span>
      </button>
      <button
        v-if="scene === 'choose'"
        class="text-button"
        :disabled="busy"
        @click="emit('skip')"
      >
        {{ zh ? '稍后添加' : 'Add a profile later' }}
      </button>
      <button
        v-else-if="scene === 'offline'"
        class="primary-button"
        form="guided-offline-profile"
        type="submit"
        :disabled="busy || !ready"
      >
        <LoadingIndicator
          v-if="busy"
          compact
          :label="t('profiles.saving')"
        /><template v-else>
          {{ duplicate ? (zh ? '使用此档案' : 'Use this profile') : (zh ? '创建档案' : 'Create profile') }}<ArrowRight aria-hidden="true" />
        </template>
      </button>
      <button
        v-else-if="scene === 'microsoft'"
        class="primary-button"
        :disabled="busy || loginStarting || loginCancelling || browserOpening"
        @click="flow ? openBrowser() : beginLogin()"
      >
        {{ flow ? (zh ? '打开浏览器' : 'Open browser') : (zh ? 'Microsoft 登录' : 'Sign in with Microsoft') }}<ExternalLink aria-hidden="true" />
      </button>
      <button
        v-else
        class="primary-button"
        :disabled="busy"
        @click="emit('complete')"
      >
        {{ zh ? '准备游戏' : 'Set up a game' }}<ArrowRight aria-hidden="true" />
      </button>
    </footer>
  </section>
</template>
