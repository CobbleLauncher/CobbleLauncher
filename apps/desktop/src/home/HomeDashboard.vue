<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import type { HomeGame } from '../library-home';
import type { PlayerProfile } from '../games';
import type { CommandAction } from '../commands/registry';
import GameCard from '../GameCard.vue';
import AppIcon from '../ui/AppIcon.vue';
import HomeContextPanel from './HomeContextPanel.vue';
import HomeGameArchive from './HomeGameArchive.vue';
import { t } from '../i18n';
const props = defineProps<{ game: HomeGame; games: HomeGame[]; archiveGames?: HomeGame[]; profile: PlayerProfile | null; active: boolean; starting: boolean; retrying: boolean; icons: Map<string,string>; covers?: Map<string,string>; coverUrl?: string; commands: CommandAction[]; toolsLoading?: boolean; showTools?: boolean; unavailableTools?: number }>();
const failedIcons = ref(new Set<string>());
const emit = defineEmits<{ select: [id:string]; play:[id:string]; manage:[id:string]; retry:[id:string]; retrySessionRecovery:[id:string]; appearance:[game:HomeGame]; library:[]; section:[section:'worlds'|'screenshots', folderName?:string]; mesh:[mode:'host'|'join']; tools:[]; configureTools:[]; visibleArchiveGames:[ids:string[]] }>();
const archiveOpen = ref(false), archiveTrigger = ref<HTMLButtonElement>();
let archiveOrigin: DOMRect | undefined;
function openArchive() {
  archiveOrigin = archiveTrigger.value?.closest('.home-launch-panel')?.querySelector('.home-launch-art')?.getBoundingClientRect();
  archiveOpen.value = true;
}
function positionArchive(element: Element) {
  if (!(element instanceof HTMLElement) || !archiveOrigin?.width) return;
  const scene = element.querySelector<HTMLElement>('.home-archive-scene');
  if (!scene) return;
  const rect = scene.getBoundingClientRect();
  scene.style.transition = 'none';
  element.style.setProperty('--archive-origin-x', `${archiveOrigin.left - rect.left + (archiveOrigin.width - rect.width) / 2}px`);
  element.style.setProperty('--archive-origin-y', `${archiveOrigin.top - rect.top + (archiveOrigin.height - rect.height) / 2}px`);
  element.style.setProperty('--archive-origin-scale-x', String(archiveOrigin.width / rect.width));
  element.style.setProperty('--archive-origin-scale-y', String(archiveOrigin.height / rect.height));
  // Establish the original 2D pose before Vue advances the enter transition.
  scene.getBoundingClientRect();
  scene.style.removeProperty('transition');
}

function closeArchive() {
  archiveOpen.value = false;
  emit('visibleArchiveGames', []);
  void nextTick(() => archiveTrigger.value?.focus({ preventScroll: true }));
}
function chooseArchiveGame(id: string) { emit('select', id); closeArchive(); }
function manageLibrary() { archiveOpen.value = false; emit('visibleArchiveGames', []); emit('library'); }
function deactivateArchive(element: Element) { if (element instanceof HTMLElement) element.inert = true; }
const switcher = ref<HTMLElement>();
function revealSelected() {
  switcher.value?.querySelector<HTMLElement>('[aria-pressed="true"]')?.scrollIntoView?.({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
}
watch(() => [props.game.id, props.games.map(game => game.id).join('|')], () => { void nextTick(revealSelected); }, { immediate: true });
function switchGame(event: KeyboardEvent, index: number) {
  let target: number;
  if (event.key === 'ArrowRight') target = Math.min(index + 1, props.games.length - 1);
  else if (event.key === 'ArrowLeft') target = Math.max(index - 1, 0);
  else if (event.key === 'Home') target = 0;
  else if (event.key === 'End') target = props.games.length - 1;
  else return;
  event.preventDefault();
  const game = props.games[target];
  if (!game) return;
  switcher.value?.querySelectorAll<HTMLButtonElement>('button')[target]?.focus({ preventScroll: true });
  emit('select', game.id);
}
function scrollGames(event: WheelEvent) {
  const strip = switcher.value;
  if (!strip || event.ctrlKey || event.shiftKey || Math.abs(event.deltaX) >= Math.abs(event.deltaY)) return;
  const delta = event.deltaY * (event.deltaMode === 1 ? 24 : event.deltaMode === 2 ? strip.clientWidth : 1);
  const next = Math.max(0, Math.min(strip.scrollWidth - strip.clientWidth, strip.scrollLeft + delta));
  if (next === strip.scrollLeft) return;
  event.preventDefault();
  strip.scrollLeft = next;
}
</script>
<template>
  <div class="home-dashboard">
    <div
      class="home-launch-panel"
      :class="{ 'is-choosing-game': archiveOpen }"
    >
      <div
        class="home-launch-front"
        :inert="archiveOpen || undefined"
        :aria-hidden="archiveOpen || undefined"
      >
        <GameCard
          :game="game"
          variant="launch"
          :profile="profile"
          :starting="starting"
          :retrying="retrying"
          :icon-url="game.icon_ref ? icons.get(game.icon_ref) : undefined"
          :cover-url="coverUrl"
          @play="$emit('play',$event)"
          @manage="$emit('manage',$event)"
          @retry="$emit('retry',$event)"
          @retry-session-recovery="$emit('retrySessionRecovery',$event)"
          @appearance="$emit('appearance',$event)"
        >
          <template #profile>
            <div class="home-launch-profile">
              <slot name="profile" />
            </div>
          </template>
        </GameCard>
        <nav
          v-if="games.length > 1"
          ref="switcher"
          class="home-game-switcher"
          :aria-label="t('home.dashboard.chooseGame')"
          @wheel="scrollGames"
        >
          <button
            v-for="(item, index) in games"
            :key="item.id"
            type="button"
            :aria-pressed="item.id === game.id"
            :tabindex="item.id === game.id ? 0 : -1"
            :title="item.name"
            :aria-label="item.name"
            @click="$emit('select',item.id)"
            @keydown="switchGame($event,index)"
          >
            <span
              class="home-instance-icon"
              :data-preset="item.appearance_preset ?? 'grass'"
            ><img
              v-if="item.icon_ref && icons.get(item.icon_ref) && !failedIcons.has(icons.get(item.icon_ref)!)"
              :src="icons.get(item.icon_ref)"
              alt=""
              @error="failedIcons.add(icons.get(item.icon_ref!)!)"
            ><svg
              v-else
              viewBox="0 0 80 80"
              aria-hidden="true"
              focusable="false"
            ><path
              class="block-top"
              d="m12 24 28-16 28 16-28 16z"
            /><path
              class="block-left"
              d="m12 24 28 16v32L12 56z"
            /><path
              class="block-right"
              d="m40 40 28-16v32L40 72z"
            /></svg><span
              v-if="item.id===game.id"
              class="home-instance-check"
            ><AppIcon
              name="check"
              :size="12"
            /></span></span><span class="home-instance-name">{{ item.name }}</span>
          </button>
        </nav>
        <div class="home-launch-footer">
          <button
            ref="archiveTrigger"
            class="text-button home-more-games"
            :aria-expanded="archiveOpen"
            @click="openArchive"
          >
            <AppIcon name="more-horizontal" />{{ t('home.dashboard.chooseMore') }}
          </button><button
            class="text-button home-manage-library"
            @click="$emit('library')"
          >
            <AppIcon name="folder" />{{ t('home.dashboard.manageLibrary') }}
          </button><slot name="add" />
        </div>
      </div>
      <Transition
        name="home-archive-turn"
        @enter="positionArchive"
        @before-leave="deactivateArchive"
      >
        <HomeGameArchive
          v-if="archiveOpen"
          :games="archiveGames ?? games"
          :selected-id="game.id"
          :icons="icons"
          :covers="covers"
          @select="chooseArchiveGame"
          @close="closeArchive"
          @library="manageLibrary"
          @visible="$emit('visibleArchiveGames',$event)"
        />
      </Transition>
    </div>
    <HomeContextPanel
      :game="game"
      :active="active"
      :commands="commands"
      :loading="toolsLoading"
      :show-tools="showTools"
      :unavailable-tools="unavailableTools"
      @section="(section, folderName) => $emit('section',section,folderName)"
      @mesh="$emit('mesh',$event)"
      @tools="$emit('tools')"
      @configure-tools="$emit('configureTools')"
    >
      <slot name="saved" />
    </HomeContextPanel>
  </div>
</template>
<style scoped>
.home-dashboard{display:grid;grid-template-columns:minmax(320px,.85fr) minmax(0,1.15fr);gap:32px;align-items:start}.home-launch-panel{position:relative;perspective:1000px;min-width:0;overflow:hidden;border-radius:12px;background:var(--ui-surface-1);box-shadow:0 4px 20px color-mix(in srgb,var(--ui-text) 5%,transparent)}.home-game-switcher{display:flex;gap:6px;margin:0 16px;padding:12px 0 6px;border-top:1px solid var(--ui-border);overflow-x:auto;overscroll-behavior-x:contain;scrollbar-width:thin;scroll-padding-inline:2px}.home-game-switcher button{flex:0 0 calc((100% - 12px)/3);min-width:72px;display:flex;flex-direction:column;align-items:center;gap:8px;padding:10px 6px;border:0;border-radius:8px;background:transparent;color:var(--ui-text);font:inherit;font-size:var(--ui-text-caption)}.home-game-switcher button:hover,.home-game-switcher button[aria-pressed=true]{background:var(--tint)}.home-instance-icon{display:grid;place-items:center;position:relative;flex:none;width:36px;height:36px;border-radius:7px;background:var(--ui-surface-2);color:var(--ui-focus)}.home-game-switcher button:focus-visible{outline:2px solid var(--ui-focus);outline-offset:-2px}.home-instance-icon>svg{width:32px;height:32px;stroke:none}.home-instance-icon{--block-top:#b4d889;--block-left:#8ba85e;--block-right:#668244}.block-top{fill:var(--block-top)}.block-left{fill:var(--block-left)}.block-right{fill:var(--block-right)}.home-instance-icon[data-preset=stone]{--block-top:#bfc5c3;--block-left:#969f9e;--block-right:#748080}.home-instance-icon[data-preset=dirt]{--block-top:#c4a17c;--block-left:#aa8059;--block-right:#856344}.home-instance-icon[data-preset=oak]{--block-top:#d6bd83;--block-left:#b99a61;--block-right:#997d4a}.home-instance-icon[data-preset=water]{--block-top:#91c5df;--block-left:#6cabc9;--block-right:#4888ad}.home-instance-icon[data-preset=lava]{--block-top:#efb470;--block-left:#dc8f4f;--block-right:#bc6939}.home-instance-icon[data-preset=end]{--block-top:#c3b1df;--block-left:#a38abd;--block-right:#82689c}.home-instance-icon img{width:36px;height:36px;object-fit:cover;border-radius:7px}.home-instance-check{position:absolute;right:-4px;bottom:-3px;display:grid;place-items:center;width:18px;height:18px;border-radius:50%;background:var(--ui-accent);color:var(--ui-on-accent);border:2px solid var(--tint)}.home-instance-name{max-width:100%;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.home-launch-footer{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:12px;padding:8px 16px 16px}.home-launch-footer>button{display:flex;align-items:center;gap:8px;min-height:var(--ui-control-height);font-size:var(--ui-text-caption)}.home-launch-profile{margin-bottom:12px;min-height:var(--ui-control-height)}
 .home-launch-front{transition:opacity 220ms ease}.is-choosing-game .home-launch-front{opacity:0}.home-archive-turn-enter-active,.home-archive-turn-leave-active{transition:opacity 360ms cubic-bezier(.2,.75,.2,1)}.home-archive-turn-enter-from,.home-archive-turn-leave-to{opacity:0}
@media(prefers-reduced-motion:reduce){.home-launch-front,.home-archive-turn-enter-active,.home-archive-turn-leave-active{transition:none}}
[data-motion=reduced] .home-launch-front,[data-motion=reduced] .home-archive-turn-enter-active,[data-motion=reduced] .home-archive-turn-leave-active{transition:none}
@media(max-width:850px){.home-dashboard{grid-template-columns:minmax(280px,.9fr) minmax(0,1fr);gap:24px}}@media(max-width:700px){.home-dashboard{grid-template-columns:1fr;gap:24px}}
</style>
