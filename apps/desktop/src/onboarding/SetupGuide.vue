<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { ArrowLeft, ArrowRight, Boxes, Download, UserRound, UsersRound } from '@lucide/vue';
import { locale } from '../i18n';
import { guideSteps, type FirstRunStep } from '../first-run';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
import GuideScene from './GuideScene.vue';
import GameDirectorySetup from './GameDirectorySetup.vue';
const props = defineProps<{ step: Exclude<FirstRunStep, 'consent'>; busy?: boolean; error?: string }>();
const emit = defineEmits<{ step: [step: FirstRunStep]; meshChoice: [enabled: boolean]; addProfile: []; createGame: []; importLauncher: []; defer: [] }>();
const zh = computed(() => locale.value === 'zh-CN');
const index = computed(() => guideSteps.indexOf(props.step));
const icons = [Boxes, UserRound, Download, UsersRound];
const directory = ref<InstanceType<typeof GameDirectorySetup>>();
const directoryBusy = ref(false);
const directoryReady = ref(false);
const pending = computed(() => props.busy || directoryBusy.value);
const content = computed(() => ({
  welcome: { title: zh.value ? '开始使用 Cobble' : 'Welcome to Cobble', description: zh.value ? '准备档案和游戏，然后开始游玩。' : 'Set up a profile and a game, then start playing.' },
  profile: { title: zh.value ? '你在游戏里的身份' : 'Your in-game profile', description: zh.value ? '登录 Microsoft，或创建离线档案。' : 'Sign in with Microsoft or create an offline profile.' },
  game: { title: zh.value ? '准备第一个游戏' : 'Create your first game', description: zh.value ? '先选好安装位置，再选择 Minecraft 版本。' : 'Choose an installation folder, then pick a Minecraft version.' },
  mesh: { title: zh.value ? '和朋友一起玩' : 'Play with friends', description: zh.value ? 'Mesh 会在后台连接联机服务，你可以随时关闭。' : 'Mesh connects to multiplayer services in the background. Turn it off anytime.' },
})[props.step]);
async function next() {
  if (pending.value) return;
  if (props.step === 'welcome') emit('step', 'profile');
  else if (props.step === 'profile') emit('addProfile');
  else if (props.step === 'game') { if (await directory.value?.save()) emit('createGame'); }
  else emit('meshChoice', true);
}
async function importLauncher() {
  if (pending.value || !directoryReady.value) return;
  if (await directory.value?.save()) emit('importLauncher');
}
function skip() { if (props.step === 'profile') emit('step', 'game'); else if (props.step === 'game') emit('step', 'mesh'); else if (props.step === 'mesh') emit('meshChoice', false); else emit('defer'); }
function focusHeading(element: Element) { (element.querySelector('[data-step-heading]') as HTMLElement | null)?.focus({ preventScroll: true }); }
</script>
<template>
  <section
    class="onboarding-page setup-guide"
    :aria-busy="pending"
  >
    <div
      class="guide-progress"
      :aria-label="zh ? '设置进度' : 'Setup progress'"
    >
      <span
        v-for="(item, position) in guideSteps"
        :key="item"
        class="guide-progress-step"
        :class="{ current: position === index, done: position < index }"
        :aria-current="position === index ? 'step' : undefined"
      ><component
        :is="icons[position]"
        aria-hidden="true"
      /></span>
      <span class="guide-count">{{ index + 1 }} / {{ guideSteps.length }}</span>
    </div>
    <div class="guide-stage">
      <Transition
        name="setup-step"
        mode="out-in"
        @after-enter="focusHeading"
      >
        <div
          :key="step"
          class="guide-content"
          :class="{'guide-content-directory':step === 'game'}"
        >
          <GuideScene :step="step" />
          <div class="guide-copy">
            <h1
              tabindex="-1"
              data-step-heading
            >
              {{ content.title }}
            </h1>
            <p class="guide-description">
              {{ content.description }}
            </p>
            <GameDirectorySetup
              v-if="step === 'game'"
              ref="directory"
              :disabled="busy"
              @busy="directoryBusy = $event"
              @ready="directoryReady = $event"
            />
            <button
              v-if="step === 'game'"
              type="button"
              class="text-button guide-import-launcher"
              :disabled="pending || !directoryReady"
              @click="importLauncher"
            >
              <Download aria-hidden="true" />{{ zh ? '导入 PCL 等启动器游戏' : 'Import from PCL or another launcher' }}
            </button>
          </div>
        </div>
      </Transition>
    </div>
    <footer class="onboarding-footer guide-footer">
      <p
        v-if="error"
        role="alert"
        class="settings-error"
      >
        {{ error }}
      </p>
      <div class="onboarding-actions">
        <button
          v-if="index > 0"
          class="secondary-button guide-back"
          :disabled="pending"
          @click="emit('step', guideSteps[index - 1]!)"
        >
          <ArrowLeft aria-hidden="true" /><span class="guide-back-label">{{ zh ? '上一步' : 'Back' }}</span>
        </button>
        <div class="guide-forward">
          <button
            class="secondary-button"
            :disabled="pending"
            @click="skip"
          >
            {{ step === 'mesh' ? (zh ? '保持关闭' : 'Keep off') : (zh ? '稍后设置' : 'Set up later') }}
          </button>
          <button
            class="primary-button"
            :disabled="pending || (step === 'game' && !directoryReady)"
            @click="next"
          >
            <LoadingIndicator
              v-if="pending"
              compact
              :label="zh ? '请稍候' : 'Please wait'"
            />
            <template v-else>
              {{ step === 'welcome' ? (zh ? '开始设置' : 'Get started') : step === 'profile' ? (zh ? '添加档案' : 'Add profile') : step === 'game' ? (zh ? '选择游戏' : 'Choose game') : (zh ? '开启 Mesh' : 'Enable Mesh') }}<ArrowRight aria-hidden="true" />
            </template>
          </button>
        </div>
      </div>
    </footer>
  </section>
</template>

<style scoped>
.setup-guide .guide-stage{padding:0 8px}
.setup-guide .guide-content{display:grid;grid-template-columns:minmax(0,1.08fr) minmax(0,.92fr);align-items:center;gap:clamp(24px,4vw,48px);width:min(100%,820px);padding:24px 0}
.guide-copy{min-width:0}
.guide-import-launcher{display:inline-flex;align-items:center;gap:var(--ui-space-2);margin-top:var(--ui-space-3)}
.setup-guide .guide-description{margin:12px 0 0;max-width:34ch;line-height:1.65}
@media(max-width:600px){.setup-guide .guide-stage{padding:0}.setup-guide .guide-content{grid-template-columns:1fr;gap:16px;width:min(100%,420px);padding:8px 0 16px}.setup-guide .guide-content-directory :deep(.guide-scene){width:min(100%,300px)}.guide-copy{text-align:center}.setup-guide .guide-description{margin:8px auto 0;max-width:36ch}}
</style>
