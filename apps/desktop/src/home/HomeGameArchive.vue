<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { HomeGame } from '../library-home';
import { searchHomeGames } from '../library-home';
import { gameLoaderLabel } from '../games';
import AppIcon from '../ui/AppIcon.vue';
import SearchField from '../ui/SearchField.vue';
import { t } from '../i18n';
import { useArchiveMotion } from './use-archive-motion';
const props = defineProps<{ games: HomeGame[]; selectedId: string; icons: Map<string, string>; covers?: Map<string, string> }>();
const emit = defineEmits<{ select: [id: string]; close: []; library: []; visible: [ids: string[]] }>();
const root = ref<HTMLElement>();
const stack = ref<HTMLElement>();
const pageSize = 6;
const query = ref(''), page = ref(Math.floor(Math.max(0, props.games.findIndex(game => game.id === props.selectedId)) / pageSize)), activeId = ref(props.selectedId), hoveredId = ref('');
const failedImages = ref(new Set<string>());
const matches = computed(() => searchHomeGames(props.games, query.value));
const pageCount = computed(() => Math.max(1, Math.ceil(matches.value.length / pageSize)));
const visible = computed(() => matches.value.slice(page.value * pageSize, (page.value + 1) * pageSize));
const highlightedId = computed(() => hoveredId.value || activeId.value);
const highlightedIndex = computed(() => visible.value.findIndex(game => game.id === highlightedId.value));
const motion = useArchiveMotion(stack, highlightedIndex);
const browsePosition = computed(() => Math.max(0, matches.value.findIndex(game => game.id === highlightedId.value)));
let wheelAmount = 0, lastWheelAt = 0, lastWheelStep = -Infinity;
let pointerPosition: { x: number; y: number } | undefined;
function browseWithWheel(event: WheelEvent) {
  if (event.ctrlKey || window.matchMedia?.('(prefers-reduced-motion: reduce)').matches || root.value?.closest('[data-motion="reduced"]')) return;
  const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
  if (!delta) return;
  const direction = delta > 0 ? 1 : -1;
  const next = browsePosition.value + direction;
  if (next < 0 || next >= matches.value.length) { wheelAmount = 0; return; }
  event.preventDefault();
  const now = performance.now();
  if (now - lastWheelAt > 220 || Math.sign(wheelAmount) !== direction) wheelAmount = 0;
  lastWheelAt = now;
  if (now - lastWheelStep < 120) return;
  wheelAmount += delta * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 300 : 1);
  if (Math.abs(wheelAmount) < 32) return;
  wheelAmount = 0; lastWheelStep = now;
  const game = matches.value[next]!;
  page.value = Math.floor(next / pageSize);
  activeId.value = game.id; hoveredId.value = '';
}
function containsPointer(element: Element | null | undefined, event: MouseEvent) {
  if (!element) return false;
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0 && event.clientX >= rect.left && event.clientX <= rect.right && event.clientY >= rect.top && event.clientY <= rect.bottom;
}
function previewAtPointer(event: PointerEvent) {
  if (event.pointerType === 'touch') return;
  // Layout-induced pointer events must not undo the user's preview as sheets make room.
  if (pointerPosition && Math.hypot(event.clientX - pointerPosition.x, event.clientY - pointerPosition.y) < 1) return;
  pointerPosition = { x: event.clientX, y: event.clientY };
  motion.move(event);
  const cards = [...(stack.value?.querySelectorAll<HTMLElement>('.home-archive-card') ?? [])];
  const current = cards.find(card => card.dataset.gameId === highlightedId.value);
  // The opened sheet owns its visible body, including the space cleared by foreground sheets.
  if (containsPointer(current?.querySelector('.home-archive-face'), event)) return;
  const direct = (event.target as Element).closest<HTMLElement>('.home-archive-card');
  const candidate = direct ?? cards.slice().reverse().find(card => containsPointer(card.querySelector('.home-archive-face'), event));
  if (candidate?.dataset.gameId) hoveredId.value = candidate.dataset.gameId;
}
function pickExposedSheet(event: MouseEvent) {
  const current = stack.value?.querySelector('.home-archive-card.is-highlighted .home-archive-face');
  if (containsPointer(current, event)) select(highlightedId.value);
}
function pickGame(id: string, event: MouseEvent) {
  // A wheel gesture previews a different sheet beneath a stationary pointer.
  const touch = 'pointerType' in event && event.pointerType === 'touch';
  select(event.detail > 0 && !touch ? highlightedId.value : id);
}

watch(() => visible.value.map(game => game.id).join('|'), () => emit('visible', visible.value.map(game => game.id)), { immediate: true });
watch(query, () => { page.value = 0; activeId.value = matches.value[0]?.id ?? ''; hoveredId.value = ''; });
watch(() => matches.value.length, () => { page.value = Math.min(page.value, pageCount.value - 1); });
async function focusGame(index: number) {
  const target = matches.value[Math.max(0, Math.min(matches.value.length - 1, index))];
  if (!target) return;
  page.value = Math.floor(matches.value.findIndex(game => game.id === target.id) / pageSize);
  activeId.value = target.id; hoveredId.value = '';
  await nextTick();
  const button = [...(root.value?.querySelectorAll<HTMLButtonElement>('.home-archive-card') ?? [])].find(button => button.dataset.gameId === target.id);
  button?.focus({ preventScroll: true });
  const stack = button?.closest<HTMLElement>('.home-archive-stack');
  if (button && stack && stack.scrollHeight > stack.clientHeight) {
    if (button.offsetTop < stack.scrollTop) stack.scrollTop = button.offsetTop;
    else if (button.offsetTop + button.offsetHeight > stack.scrollTop + stack.clientHeight) stack.scrollTop = button.offsetTop + button.offsetHeight - stack.clientHeight;
  }
}
function keydown(event: KeyboardEvent) {
  if (event.isComposing || event.keyCode === 229) return;
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); emit('close'); return; }
  const input = event.target instanceof HTMLInputElement;
  if (!input && !(event.target instanceof Element && event.target.closest('.home-archive-card'))) return;
  if (!input && event.key === 'Enter') { event.preventDefault(); select(activeId.value); return; }
  const current = Math.max(0, matches.value.findIndex(game => game.id === activeId.value));
  let index: number;
  if (event.key === 'ArrowDown' || event.key === 'ArrowRight' && !input) index = input ? current : current + 1;
  else if (event.key === 'ArrowUp' || event.key === 'ArrowLeft' && !input) index = input ? current : current - 1;
  else if (!input && event.key === 'Home') index = 0;
  else if (!input && event.key === 'End') index = matches.value.length - 1;
  else return;
  event.preventDefault(); event.stopPropagation(); void focusGame(index);
}
function select(id: string) { if (props.games.some(game => game.id === id)) emit('select', id); }
function submit() { if (matches.value.some(game => game.id === activeId.value)) select(activeId.value); }
function changePage(direction: -1 | 1) {
  const target = Math.max(0, Math.min(pageCount.value - 1, page.value + direction));
  void focusGame(target * pageSize);
}
function image(game: HomeGame) {
  const cover = game.cover_ref ? props.covers?.get(game.cover_ref) : undefined;
  return cover && !failedImages.value.has(cover) ? cover : undefined;
}
onMounted(() => { root.value?.querySelector<HTMLInputElement>('input[type="search"]')?.focus({ preventScroll: true }); });
onBeforeUnmount(() => emit('visible', []));
</script>
<template>
  <section
    ref="root"
    class="home-game-archive"
    :aria-label="t('home.dashboard.chooseGame')"
    @keydown.capture="keydown"
  >
    <header class="home-archive-header">
      <button
        class="text-button home-archive-back"
        type="button"
        @click="$emit('close')"
      >
        <AppIcon name="arrow-left" />{{ t('home.dashboard.backToGame') }}
      </button>
      <span
        class="home-archive-count"
        role="status"
      >{{ matches.length ? browsePosition + 1 : 0 }} / {{ matches.length }}</span>
    </header>
    <SearchField
      v-model="query"
      :label="t('home.search')"
      :placeholder="t('home.search')"
      @submit="submit"
    />
    <div
      v-if="visible.length"
      ref="stack"
      class="home-archive-stack"
      :style="{ '--archive-count': visible.length }"
      @pointermove="previewAtPointer"
      @click="pickExposedSheet"
      @mouseleave="hoveredId = ''; pointerPosition = undefined; motion.leave()"
      @wheel="browseWithWheel"
    >
      <div class="home-archive-scene">
        <button
          v-for="(item, index) in visible"
          :key="item.id"
          type="button"
          class="home-archive-card"
          :class="{ 'is-highlighted': highlightedId === item.id, 'is-current': item.id === selectedId, 'is-before-highlighted': index < highlightedIndex }"
          :style="{ '--archive-index': index }"
          :data-game-id="item.id"
          :data-preset="item.appearance_preset ?? 'grass'"
          :aria-label="t('home.selectGame', { name: item.name })"
          :aria-pressed="item.id === selectedId"
          :title="item.name"
          :tabindex="item.id === activeId || !visible.some(game => game.id === activeId) && index === 0 ? 0 : -1"
          @mousedown.prevent
          @focus="activeId = item.id; hoveredId = ''"
          @click.stop="pickGame(item.id, $event)"
        >
          <span class="home-archive-face">
            <span
              class="home-archive-tab"
              aria-hidden="true"
            ><span class="home-archive-number">{{ String(index + 1).padStart(2, '0') }}</span></span>
            <img
              v-if="image(item)"
              class="home-archive-cover"
              :src="image(item)"
              alt=""
              @error="failedImages.add(($event.target as HTMLImageElement).src)"
            >
            <span
              class="home-archive-glyph"
              aria-hidden="true"
            >
              <img
                v-if="item.icon_ref && icons.get(item.icon_ref) && !failedImages.has(icons.get(item.icon_ref)!)"
                :src="icons.get(item.icon_ref)"
                alt=""
                @error="failedImages.add(($event.target as HTMLImageElement).src)"
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
            </span>
            <span class="home-archive-caption"><strong>{{ item.name }}</strong><span>{{ item.version }} · {{ item.loader_version ? gameLoaderLabel(item) : t('home.vanilla') }}</span></span>
            <AppIcon
              v-if="item.id === selectedId"
              name="check"
              class="home-archive-selected"
            />
          </span>
        </button>
      </div>
    </div>
    <p
      v-else
      class="home-archive-empty"
      role="status"
    >
      {{ t('home.noMatches') }}
    </p>
    <footer class="home-archive-footer">
      <button
        type="button"
        class="text-button home-archive-manage-library"
        @click="$emit('library')"
      >
        <AppIcon name="folder" />{{ t('home.dashboard.manageLibrary') }}
      </button>
      <div
        v-if="matches.length > pageSize"
        class="home-archive-pages"
      >
        <button
          type="button"
          class="text-button"
          :aria-label="t('home.previous')"
          :disabled="page === 0"
          @click="changePage(-1)"
        >
          <AppIcon
            name="chevron-right"
            class="home-archive-previous"
          />
        </button>
        <span role="status">{{ page + 1 }} / {{ pageCount }}</span>
        <button
          type="button"
          class="text-button"
          :aria-label="t('home.next')"
          :disabled="page >= pageCount - 1"
          @click="changePage(1)"
        >
          <AppIcon name="chevron-right" />
        </button>
      </div>
    </footer>
  </section>
</template>
<style scoped src="./HomeGameArchive.css" />
