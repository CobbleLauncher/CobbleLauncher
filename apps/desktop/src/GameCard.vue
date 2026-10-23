<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { ContextMenuRoot, ContextMenuTrigger } from 'reka-ui';
import type { PlayerProfile } from './games';
import type { HomeGame } from './library-home';
import { gameLoaderLabel } from './games';
import { t } from './i18n';
import GameCardMenu from './GameCardMenu.vue';
import AppIcon from './ui/AppIcon.vue';
const props = withDefaults(defineProps<{ game: HomeGame; retrying: boolean; profile: PlayerProfile | null; starting: boolean; variant?: 'featured' | 'row'; iconUrl?: string | null; coverUrl?: string | null }>(), { variant: 'featured', iconUrl: null, coverUrl: null });
defineEmits<{ retry: [id: string]; play: [id: string]; manage: [id: string]; appearance: [game: HomeGame] }>();
const brokenIcon = ref(false);
const brokenCover = ref(false);
const contextOpen = ref(false);
function openKeyboardMenu(event: KeyboardEvent) {
  if (props.game.status === 'installing' || !(event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10'))) return;
  event.preventDefault();
  contextOpen.value = true;
}
watch(() => props.coverUrl, () => { brokenCover.value = false; });
const lastPlayed = computed(() => {
  const time = props.game.last_launched_at;
  return time && Number.isFinite(time) && time > 0 ? t('home.lastPlayed', { date: new Date(time).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) }) : t('home.neverPlayed');
});
watch(() => props.iconUrl, () => { brokenIcon.value = false; });
const loader = computed(() => props.game.loader_version ? gameLoaderLabel(props.game) : t('home.vanilla'));
const running = computed(() => props.game.launch_state === 'running');
const preparing = computed(() => props.starting || props.game.launch_state === 'preparing');
const playLabel = computed(() => props.game.content_installing ? t('home.contentBusy') : running.value ? t('home.running') : preparing.value ? t('home.starting') : props.profile ? t('home.play') : t('home.chooseProfile'));
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
  <ContextMenuRoot v-model:open="contextOpen">
    <ContextMenuTrigger
      as-child
      :disabled="game.status === 'installing'"
    >
      <article
        class="game-card library-game"
        :class="[`library-game--${variant}`, { 'has-cover': variant === 'featured' && coverUrl && !brokenCover }]"
        :aria-label="game.name"
        :data-preset="preset"
        tabindex="-1"
        @keydown="openKeyboardMenu"
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
          <p
            v-if="variant === 'row' && (running || preparing || game.content_installing)"
            class="library-game-state"
            role="status"
          >
            {{ game.content_installing ? t('home.contentBusy') : running ? t('home.running') : t('home.starting') }}
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
            v-if="game.status === 'failed' || game.launch_error"
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
            v-else
            class="primary-button library-game-play"
            :disabled="game.content_installing || preparing || running"
            :aria-label="profile ? t('home.playGame', { name: game.name }) : t('home.chooseProfileFor', { name: game.name })"
            @click="$emit('play', game.id)"
          >
            <AppIcon name="play" /><span class="play-label">{{ playLabel }}</span>
          </button>
          <GameCardMenu
            :game="game"
            @updated="$emit('appearance', $event)"
            @manage="$emit('manage', $event)"
          />
        </div>
      </article>
    </ContextMenuTrigger>
  </ContextMenuRoot>
</template>
<style scoped>
.library-game-open{padding:0;border:0;background:transparent;font:inherit;color:inherit;text-align:left;cursor:pointer}.library-game-open:hover{text-decoration:underline;text-underline-offset:4px}.library-game-pinned{position:absolute;top:10px;left:10px;display:grid;place-items:center;color:var(--muted);z-index:1}
.library-game{display:grid;grid-template-columns:76px minmax(0,1fr) auto;gap:22px;padding:26px;min-height:160px;animation:none}.library-game-cover{width:76px;height:76px;display:grid;place-items:center;flex-shrink:0;border-radius:16px;background:var(--hover);overflow:hidden;--block-top:#b4d889;--block-left:#8ba85e;--block-right:#668244}.library-game-cover svg{width:68px;height:68px;stroke:none}.library-game-cover img{width:100%;height:100%;object-fit:cover;image-rendering:pixelated}.block-top{fill:var(--block-top)}.block-left{fill:var(--block-left)}.block-right{fill:var(--block-right)}.library-game-cover[data-preset=stone]{--block-top:#bfc5c3;--block-left:#969f9e;--block-right:#748080}.library-game-cover[data-preset=dirt]{--block-top:#c4a17c;--block-left:#aa8059;--block-right:#856344}.library-game-cover[data-preset=oak]{--block-top:#d6bd83;--block-left:#b99a61;--block-right:#997d4a}.library-game-cover[data-preset=water]{--block-top:#91c5df;--block-left:#6cabc9;--block-right:#4888ad}.library-game-cover[data-preset=lava]{--block-top:#efb470;--block-left:#dc8f4f;--block-right:#bc6939}.library-game-cover[data-preset=end]{--block-top:#c3b1df;--block-left:#a38abd;--block-right:#82689c}.library-game-body{min-width:0}.library-game .library-game-name{font-size:22px;line-height:1.35;font-weight:650;letter-spacing:-.5px;margin:0;overflow-wrap:anywhere}.library-game-version{font-size:12px;color:var(--muted);margin-top:7px;line-height:1.5}.library-game-profile{font-size:11px;color:var(--muted);margin-top:10px}.library-game-actions{display:flex;align-items:center;gap:10px}.library-game-play{min-width:118px;padding:0 18px;min-height:42px}.play-label{font-size:12px;font-weight:600}.library-game-retry{min-height:38px;padding:0 14px}.library-game-error{font-size:11px;color:var(--danger,#b74332);margin-top:8px}.library-game-progress-label{display:flex;justify-content:space-between;gap:12px;margin-top:12px;font-size:11px;color:var(--muted)}.library-game-progress{height:4px;border-radius:4px;background:var(--hover);overflow:hidden;margin-top:6px;max-width:340px}.library-game-progress>div{height:100%;background:#8caf60;border-radius:4px}.library-game-progress.indeterminate>div{width:35%;animation:home-install 1.4s ease-in-out infinite alternate}.library-game--row{grid-template-columns:44px minmax(0,1fr) auto;padding:16px 18px;gap:14px;min-height:82px;border-radius:12px}.library-game--row .library-game-cover{width:44px;height:44px;border-radius:10px}.library-game--row .library-game-cover svg{width:40px;height:40px}.library-game--row .library-game-name{font-size:14px;letter-spacing:0;font-weight:600}.library-game--row .library-game-version{font-size:11px;margin-top:4px}.library-game--row .library-game-play{min-width:84px;min-height:36px;padding:0 13px}.library-game-state{color:var(--muted);font-size:10px;margin-top:5px}.library-game--row .library-game-actions{gap:5px}@keyframes home-install{to{transform:translateX(180%)}}@media(max-width:560px){.library-game{grid-template-columns:58px minmax(0,1fr);padding:20px;gap:13px;min-height:160px}.library-game-cover{width:58px;height:58px;border-radius:12px}.library-game-cover svg{width:53px;height:53px}.library-game .library-game-name{font-size:19px}.library-game-actions{grid-column:2;gap:8px}.library-game-profile{margin-top:6px}.library-game--row{grid-template-columns:40px minmax(0,1fr) auto;gap:11px;min-height:80px;padding:14px}.library-game--row .library-game-cover{width:40px;height:40px}.library-game--row .library-game-name{font-size:14px}.library-game--row .library-game-actions{grid-column:auto;gap:3px}.library-game--row .library-game-play{min-width:36px;padding:0 9px}.library-game--row .play-label{display:none}.library-game--row .library-game-retry{font-size:10px;min-height:35px;padding:0 8px}}
@media(min-width:561px){.library-game-state{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap}}:global(:root[data-theme=dark]) .library-game-error{color:var(--danger,#ffa093)}@media(prefers-color-scheme:dark){:global(:root:not([data-theme=light])) .library-game-error{color:var(--danger,#ffa093)}}
.library-game{position:relative;isolation:isolate;overflow:hidden;border-radius:var(--ui-radius-3,12px);gap:24px;padding:32px;min-height:216px;align-items:center}.library-game--featured:before{content:"";position:absolute;inset:0;z-index:-1;background:linear-gradient(110deg,var(--panel) 45%,color-mix(in srgb,var(--accent) 10%,var(--panel)));pointer-events:none}.library-game--featured:after{content:"";position:absolute;width:240px;height:240px;right:15%;top:-110px;transform:rotate(35deg);border:38px solid color-mix(in srgb,var(--accent) 6%,transparent);z-index:-1;pointer-events:none}.library-game-backdrop{position:absolute;inset:0;width:100%;height:100%;object-fit:cover;z-index:-2}.library-game--featured.has-cover:before{background:linear-gradient(90deg,var(--panel) 15%,color-mix(in srgb,var(--panel) 94%,transparent) 45%,color-mix(in srgb,var(--panel) 60%,transparent))}.library-game--featured.has-cover:after{display:none}.library-game .library-game-name{font-size:var(--ui-text-page,24px);font-weight:600;letter-spacing:normal}.library-game-version,.library-game-last-played,.library-game-profile{font-size:12px;line-height:1.6;color:var(--muted)}.library-game-last-played{margin-top:12px}.library-game--row{min-height:96px;padding:16px 20px;gap:16px}.library-game--row .library-game-name{font-size:14px;font-weight:600}.library-game--row .library-game-last-played{margin-top:4px;font-size:12px}.library-game--row .library-game-version{font-size:12px}.library-game-play{min-height:40px;border-radius:8px}.library-game-cover{border-radius:12px}.library-game-profile{margin-top:4px}@media(max-width:560px){.library-game{padding:20px;min-height:200px;gap:16px}.library-game .library-game-name{font-size:20px}.library-game--row{padding:16px;min-height:100px;gap:12px}.library-game--row .library-game-name{font-size:14px}.library-game--row .library-game-last-played{display:none}.library-game--featured .library-game-actions{grid-column:1 / -1}.library-game--row .library-game-actions{gap:2px}}
</style>


