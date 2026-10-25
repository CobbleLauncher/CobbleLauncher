<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, shallowRef, watch } from 'vue';
import { ContextMenuRoot, ContextMenuTrigger } from 'reka-ui';
import type { PlayerProfile } from './games';
import { homeLaunchState, type HomeGame } from './library-home';
import { gameLoaderLabel } from './games';
import { locale, t } from './i18n';
import GameCardMenu from './GameCardMenu.vue';
import AppIcon from './ui/AppIcon.vue';
import type { AppIconName } from './ui/icons';
import { readCoverFocus, writeCoverFocus } from './cover-focus';
import { isGameCardSurface } from './game-card-interaction';
const props = withDefaults(defineProps<{ game: HomeGame; retrying: boolean; profile: PlayerProfile | null; starting: boolean; variant?: 'featured' | 'row'; iconUrl?: string | null; coverUrl?: string | null; sharedContext?: boolean; sharedContextOpen?: boolean }>(), { variant: 'featured', iconUrl: null, coverUrl: null });
const emit = defineEmits<{ retry: [id: string]; retrySessionRecovery: [id: string]; play: [id: string]; manage: [id: string]; appearance: [game: HomeGame]; select: [id: string] }>();
function clickCard(event: MouseEvent) {
  emit('select', props.game.id);
  if (props.game.status === 'installed' && isGameCardSurface(event)) emit('manage', props.game.id);
}
const brokenIcon = ref(false);
const brokenCover = ref(false);
const editingCover = ref(false);
const focus = ref(readCoverFocus(props.game.id, props.game.cover_ref));
watch(() => [props.game.id, props.game.cover_ref], () => { focus.value = readCoverFocus(props.game.id, props.game.cover_ref); editingCover.value = false; });
function changeFocus(axis: 'x' | 'y', event: Event) {
  focus.value = { ...focus.value, [axis]: Number((event.target as HTMLInputElement).value) };
  writeCoverFocus(props.game.id, props.game.cover_ref, focus.value);
}
function resetFocus() { focus.value = { x: 50, y: 50 }; writeCoverFocus(props.game.id, props.game.cover_ref, focus.value); }
const contextOpen = ref(false);
const contextReturnTarget = shallowRef<HTMLElement>();
function rememberContextTarget(event: MouseEvent) {
  const card = event.currentTarget as HTMLElement;
  const target = event.target instanceof Element ? event.target.closest<HTMLElement>('button,input,select,[tabindex]') : null;
  contextReturnTarget.value = target && card.contains(target) ? target : card;
}
function openKeyboardMenu(event: KeyboardEvent) {
  if (props.game.status === 'installing' || !(event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10'))) return;
  event.preventDefault();
  const target = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>('.library-game') : null;
  if (!target) return;
  const bounds = target.getBoundingClientRect();
  const source = event.target instanceof HTMLElement ? event.target : target;
  source.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: bounds.left + bounds.width / 2, clientY: bounds.top + bounds.height / 2 }));
}
watch(() => props.coverUrl, () => { brokenCover.value = false; });
const lastPlayed = computed(() => {
  const time = props.game.last_launched_at;
  return time && Number.isFinite(time) && time > 0 ? t('home.lastPlayed', { date: new Date(time).toLocaleDateString(locale.value, { month: 'short', day: 'numeric' }) }) : t('home.neverPlayed');
});
watch(() => props.iconUrl, () => { brokenIcon.value = false; });
const loader = computed(() => props.game.loader_version ? gameLoaderLabel(props.game) : t('home.vanilla'));
const launchState = computed(() => homeLaunchState(props.game));
const running = computed(() => launchState.value === 'running');
const verifying = computed(() => launchState.value === 'verifying');
const recoveryFailed = computed(() => launchState.value === 'recovery_failed');
const preparing = computed(() => props.starting || launchState.value === 'preparing');
const playLabel = computed(() => props.game.content_installing ? t('home.contentBusy') : verifying.value ? t('home.sessionVerifying') : running.value ? t('home.running') : preparing.value ? t('home.starting') : props.profile ? t('home.play') : t('home.chooseProfile'));
const rowPlayIcon = computed<AppIconName>(() => running.value ? 'check' : verifying.value || preparing.value || props.game.content_installing ? 'loader-circle' : 'play');
const playAriaLabel = computed(() => {
  if (props.variant === 'row' && verifying.value) return t('home.checkingSessionFor', { name: props.game.name });
  if (props.variant === 'row' && (running.value || preparing.value || props.game.content_installing)) return `${props.game.name} — ${playLabel.value}`;
  return props.profile ? t('home.playGame', { name: props.game.name }) : t('home.chooseProfileFor', { name: props.game.name });
});
const phase = computed(() => {
  const value = props.game.progress?.phase.replace(/^pack-base-/, '') ?? 'metadata';
  if (value === 'forge-installer') return t('home.phase.forge');
  if (value === 'neoforge-installer') return t('home.phase.neoforge');
  if (value.startsWith('pack-')) return t('home.phase.pack');
  if (value === 'import-copy') return t('home.phase.import');
  if (value === 'metadata' || value === 'files' || value === 'assets' || value === 'fabric' || value === 'complete') return t(`home.phase.${value}`);
  return t('home.phase.installing');
});
const percent = computed(() => { const value = props.game.progress; return value && value.total > 0 ? Math.max(0, Math.min(100, value.completed / value.total * 100)) : null; });
const preset = computed(() => ['grass', 'stone', 'dirt', 'oak', 'water', 'lava', 'end'].includes(props.game.appearance_preset ?? '') ? props.game.appearance_preset : 'grass');
</script>
<template>
  <component
    :is="sharedContext ? 'div' : ContextMenuRoot"
    v-model:open="contextOpen"
    :style="sharedContext ? { display: 'contents' } : undefined"
  >
    <component
      :is="sharedContext ? 'div' : ContextMenuTrigger"
      :style="sharedContext ? { display: 'contents' } : undefined"
      as-child
      :disabled="game.status === 'installing'"
    >
      <article
        class="game-card library-game"
        :class="[`library-game--${variant}`, { 'has-cover': variant === 'featured' && coverUrl && !brokenCover }]"
        :aria-label="game.name"
        :data-preset="preset"
        :data-game-status="game.status"
        :data-game-id="game.id"
        tabindex="-1"
        @focusin="$emit('select', game.id)"
        @click="clickCard"
        @keydown="openKeyboardMenu"
        @contextmenu.capture="rememberContextTarget"
      >
        <span
          v-if="game.pinned"
          class="library-game-pinned"
          role="img"
          :aria-label="t('home.pinned')"
          :title="t('home.pinned')"
        ><AppIcon
          name="pin"
          :size="14"
        /></span>
        <img
          v-if="variant === 'featured' && coverUrl && !brokenCover"
          class="library-game-backdrop"
          :src="coverUrl"
          :style="{ objectPosition: `${focus.x}% ${focus.y}%` }"
          alt=""
          @error="brokenCover = true"
        >
        <div
          class="library-game-cover"
          :data-preset="preset"
          aria-hidden="true"
        >
          <img
            v-if="iconUrl && !brokenIcon"
            :src="iconUrl"
            alt=""
            @error="brokenIcon = true"
          >
          <svg
            v-else
            viewBox="0 0 80 80"
          ><path
            class="block-top"
            d="m12 24 28-16 28 16-28 16z"
          /><path
            class="block-left"
            d="m12 24 28 16v32L12 56z"
          /><path
            class="block-right"
            d="m40 40 28-16v32L40 72z"
          /></svg>
        </div>
        <div class="library-game-body">
          <component
            :is="variant === 'featured' ? 'h2' : 'h3'"
            class="library-game-name"
          >
            <button
              v-if="game.status === 'installed'"
              class="library-game-open"
              :aria-label="t('home.manage', { name: game.name })"
              @click="$emit('manage', game.id)"
            >
              {{ game.name }}
            </button>
            <template v-else>
              {{ game.name }}
            </template>
          </component>
          <p class="library-game-version">
            {{ game.version }} · {{ loader }}
          </p>
          <p class="library-game-last-played">
            {{ lastPlayed }}
          </p>
          <p
            v-if="variant === 'featured' && profile && game.profile_id"
            class="library-game-profile"
          >
            {{ profile.name }}
          </p>
          <div
            v-if="editingCover && variant === 'featured' && coverUrl && !brokenCover"
            class="library-cover-focus"
            data-card-control
            role="group"
            :aria-label="t('home.adjustCover')"
          >
            <div
              class="library-cover-focus-controls"
            >
              <label>{{ t('home.coverHorizontal') }}<input
                type="range"
                min="0"
                max="100"
                :value="focus.x"
                @input="changeFocus('x', $event)"
              ></label>
              <label>{{ t('home.coverVertical') }}<input
                type="range"
                min="0"
                max="100"
                :value="focus.y"
                @input="changeFocus('y', $event)"
              ></label>
              <button
                class="text-button"
                @click="resetFocus"
              >
                {{ t('home.resetCoverCrop') }}
              </button>
              <button
                class="text-button"
                @click="editingCover = false"
              >
                {{ t('home.done') }}
              </button>
            </div>
          </div>
          <p
            v-if="(variant === 'row' && (running || preparing || game.content_installing)) || verifying || recoveryFailed"
            class="library-game-state"
            :class="{ 'library-game-state-recovery': verifying || recoveryFailed }"
            :role="recoveryFailed ? 'alert' : 'status'"
          >
            {{ recoveryFailed ? t('home.sessionRecoveryFailed') : verifying ? t('home.sessionVerifying') : game.content_installing ? t('home.contentBusy') : running ? t('home.running') : t('home.starting') }}
          </p>
          <template v-if="game.status === 'installing'">
            <div
              class="library-game-progress-label"
              role="status"
            >
              <span>{{ phase }}</span><span v-if="percent !== null">{{ Math.floor(percent) }}%</span>
            </div>
            <div
              class="library-game-progress"
              role="progressbar"
              :aria-label="phase"
              :aria-valuenow="percent ?? undefined"
              :aria-valuemax="100"
              :class="{ indeterminate: percent === null }"
            >
              <div :style="percent !== null ? { width: `${percent}%` } : undefined" />
            </div>
          </template>
          <p
            v-if="game.status === 'failed' || (game.launch_error && !recoveryFailed)"
            class="library-game-error"
            role="alert"
          >
            {{ game.status === 'failed' ? t('home.installFailed') : t('home.launchFailed') }}
          </p>
        </div>
        <div
          v-if="game.status !== 'installing'"
          class="library-game-actions"
        >
          <button
            v-if="game.status === 'failed'"
            class="secondary-button library-game-retry"
            :disabled="retrying"
            @click="$emit('retry', game.id)"
          >
            <AppIcon name="refresh-cw" />{{ retrying ? t('home.retrying') : t('home.retry') }}
          </button>
          <button
            v-else-if="recoveryFailed"
            class="secondary-button library-game-retry"
            :disabled="retrying"
            @click="$emit('retrySessionRecovery', game.id)"
          >
            <AppIcon name="refresh-cw" />{{ retrying ? t('home.checkingSession') : t('home.retrySessionCheck') }}
          </button>
          <button
            v-else
            class="primary-button library-game-play"
            :disabled="game.content_installing || preparing || running || verifying"
            :aria-label="playAriaLabel"
            :title="playAriaLabel"
            @click="$emit('play', game.id)"
          >
            <AppIcon
              :name="variant === 'row' ? rowPlayIcon : 'play'"
              :class="{ 'ui-busy-icon': variant === 'row' && rowPlayIcon === 'loader-circle' }"
            /><span class="play-label">{{ playLabel }}</span>
          </button>
          <GameCardMenu
            :game="game"
            :context-open="sharedContext ? sharedContextOpen : contextOpen"
            :context-return-target="contextReturnTarget"
            :adjust-cover="variant === 'featured' && !!coverUrl && !brokenCover"
            @adjust-cover="editingCover = true"
            @updated="$emit('appearance', $event)"
            @manage="$emit('manage', $event)"
          />
        </div>
      </article>
    </component>
  </component>
</template>
<style scoped>
.library-game{position:relative;isolation:isolate;display:grid;grid-template-columns:76px minmax(0,1fr) auto;align-items:center;gap:var(--ui-space-5);padding:var(--ui-space-6);min-height:calc(4 * var(--ui-control-height) + var(--ui-space-5));overflow:hidden;border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-1);animation:none}
.library-game[data-game-status=installed]{cursor:pointer;transition:border-color var(--ui-duration-fast)}.library-game[data-game-status=installed]:hover{border-color:var(--ui-border-strong)}
.library-game-open{padding:0;border:0;background:transparent;font:inherit;color:inherit;text-align:left;cursor:pointer}.library-game-open:hover{text-decoration:underline;text-underline-offset:4px}.library-game-pinned{position:absolute;top:var(--ui-space-2);left:var(--ui-space-2);display:grid;place-items:center;color:var(--ui-text-muted);z-index:1}
.library-game--featured:before{content:"";position:absolute;inset:0;z-index:-1;background:linear-gradient(110deg,var(--ui-surface-1) 45%,color-mix(in srgb,var(--ui-accent) 8%,var(--ui-surface-1)));pointer-events:none}.library-game-backdrop{position:absolute;inset:0;width:100%;height:100%;object-fit:cover;z-index:-2}.library-game--featured.has-cover:before{background:linear-gradient(90deg,var(--ui-surface-1) 15%,color-mix(in srgb,var(--ui-surface-1) 94%,transparent) 45%,color-mix(in srgb,var(--ui-surface-1) 60%,transparent))}
.library-game-cover{width:76px;height:76px;display:grid;place-items:center;flex:none;border-radius:var(--ui-radius-lg);background:var(--ui-surface-2);overflow:hidden;--block-top:#b4d889;--block-left:#8ba85e;--block-right:#668244}.library-game-cover svg{width:68px;height:68px;stroke:none}.library-game-cover img{width:100%;height:100%;object-fit:cover;image-rendering:pixelated}.block-top{fill:var(--block-top)}.block-left{fill:var(--block-left)}.block-right{fill:var(--block-right)}.library-game-cover[data-preset=stone]{--block-top:#bfc5c3;--block-left:#969f9e;--block-right:#748080}.library-game-cover[data-preset=dirt]{--block-top:#c4a17c;--block-left:#aa8059;--block-right:#856344}.library-game-cover[data-preset=oak]{--block-top:#d6bd83;--block-left:#b99a61;--block-right:#997d4a}.library-game-cover[data-preset=water]{--block-top:#91c5df;--block-left:#6cabc9;--block-right:#4888ad}.library-game-cover[data-preset=lava]{--block-top:#efb470;--block-left:#dc8f4f;--block-right:#bc6939}.library-game-cover[data-preset=end]{--block-top:#c3b1df;--block-left:#a38abd;--block-right:#82689c}
.library-game-body{min-width:0}.library-game .library-game-name{font-size:var(--ui-text-page);line-height:var(--ui-leading-page);font-weight:600;letter-spacing:normal;margin:0;overflow-wrap:anywhere}.library-game-version,.library-game-last-played,.library-game-profile{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.library-game-version{margin-top:var(--ui-space-1)}.library-game-last-played{margin-top:var(--ui-space-3)}.library-game-profile{margin-top:var(--ui-space-1)}
.library-game-actions{display:flex;align-items:center;gap:var(--ui-space-2)}.library-game-play{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-width:118px;padding:0 var(--ui-space-4);min-height:var(--ui-control-height);border-radius:var(--ui-radius-md)}.library-game-play .play-label{font-size:var(--ui-text-label);line-height:var(--ui-leading-label);font-weight:600}.library-game-retry{min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label)}.library-game-error{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-danger);margin-top:var(--ui-space-2)}.library-game-state{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted);margin-top:var(--ui-space-1)}
.library-game-progress-label{display:flex;justify-content:space-between;gap:var(--ui-space-3);margin-top:var(--ui-space-3);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-variant-numeric:tabular-nums;color:var(--ui-text-muted)}.library-game-progress{height:4px;border-radius:var(--ui-radius-sm);background:var(--ui-surface-3);overflow:hidden;margin-top:var(--ui-space-1);max-width:340px}.library-game-progress>div{height:100%;background:var(--ui-focus);border-radius:inherit}.library-game-progress.indeterminate>div{width:35%;animation:home-install 1.4s ease-in-out infinite alternate}@keyframes home-install{to{transform:translateX(180%)}}
.library-game--row{grid-template-columns:calc(var(--ui-control-height) + var(--ui-space-1)) minmax(0,1fr) auto;min-height:calc(2 * var(--ui-control-height) + var(--ui-space-4));padding:var(--ui-space-4) var(--ui-space-5);gap:var(--ui-space-4)}.library-game--row .library-game-cover{width:calc(var(--ui-control-height) + var(--ui-space-1));height:calc(var(--ui-control-height) + var(--ui-space-1));border-radius:var(--ui-radius-md)}.library-game--row .library-game-cover svg{width:var(--ui-control-height);height:var(--ui-control-height)}.library-game--row .library-game-name{font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.library-game--row .library-game-last-played{margin-top:var(--ui-space-1)}.library-game--row .library-game-play{width:var(--ui-control-height);min-width:var(--ui-control-height);padding:0}.library-game--row .play-label{display:none}.library-game--row .library-game-actions{gap:var(--ui-space-1)}
.library-cover-focus{position:relative;margin-top:var(--ui-space-2)}.library-cover-focus>.text-button{font-size:var(--ui-text-caption);padding:var(--ui-space-1) 0}.library-cover-focus-controls{display:flex;flex-wrap:wrap;align-items:center;gap:var(--ui-space-2);padding:var(--ui-space-2);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1)}.library-cover-focus-controls label{display:flex;align-items:center;gap:var(--ui-space-1);font-size:var(--ui-text-caption)}.library-cover-focus-controls input{width:80px;accent-color:var(--ui-focus)}
@media(min-width:561px){.library-game-state:not(.library-game-state-recovery){position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap}}
@media(max-width:560px){.library-game{grid-template-columns:58px minmax(0,1fr);padding:var(--ui-space-4);gap:var(--ui-space-4);min-height:calc(4 * var(--ui-control-height) + var(--ui-space-4))}.library-game-cover{width:58px;height:58px}.library-game-cover svg{width:53px;height:53px}.library-game--featured .library-game-name{font-size:var(--ui-text-section);line-height:var(--ui-leading-section)}.library-game--featured .library-game-actions{grid-column:1/-1}.library-game--row{grid-template-columns:var(--ui-control-height) minmax(0,1fr) auto;min-height:calc(2 * var(--ui-control-height) + var(--ui-space-4));gap:var(--ui-space-3)}.library-game--row .library-game-cover{width:var(--ui-control-height);height:var(--ui-control-height)}.library-game--row .library-game-last-played{display:none}.library-game--row .library-game-retry{padding:0 var(--ui-space-2)}}
</style>
