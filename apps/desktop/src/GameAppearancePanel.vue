<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game } from './games';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';

const props = defineProps<{ game: Game }>();
const emit = defineEmits<{ updated: [game: Game] }>();
type Preset = NonNullable<Game['appearance_preset']>;
const presets: { id: Preset; color: string }[] = [
  { id: 'grass', color: '#86a36c' }, { id: 'stone', color: '#8b9096' },
  { id: 'dirt', color: '#a67c5b' }, { id: 'oak', color: '#c39762' },
  { id: 'water', color: '#5e90af' }, { id: 'lava', color: '#ca7957' },
  { id: 'end', color: '#9b8eae' },
];
const busy = ref(false);
const error = ref('');
let disposed = false;
async function update(command: string, extra: Record<string, string> = {}) {
  if (busy.value) return;
  const id = props.game.id;
  busy.value = true; error.value = '';
  try {
    const game = await invoke<Game | null>(command, { id, ...extra });
    if (game && !disposed && props.game.id === id) emit('updated', game);
  } catch (cause) { if (!disposed && props.game.id === id) error.value = String(cause); }
  finally { busy.value = false; }
}
watch(() => props.game.id, () => { error.value = ''; });
onUnmounted(() => { disposed = true; });
</script>
<template>
  <SettingsDisclosure
    class="game-appearance-panel"
    bordered
    :title="t('gameAppearance.title')"
    :aria-busy="busy"
  >
    <div class="appearance-controls">
      <div
        class="appearance-presets"
        role="group"
        :aria-label="t('gameAppearance.presets')"
      >
        <button
          v-for="preset in presets"
          :key="preset.id"
          type="button"
          class="appearance-preset"
          :aria-pressed="!game.icon_ref && (game.appearance_preset ?? 'grass') === preset.id"
          :disabled="busy"
          @click="update('set_game_appearance_preset', { preset: preset.id })"
        >
          <span
            class="appearance-swatch"
            :style="{ backgroundColor: preset.color }"
            aria-hidden="true"
          />
          {{ t(`gameAppearance.${preset.id}`) }}
        </button>
      </div>
      <div class="appearance-actions">
        <button
          type="button"
          class="secondary-button"
          :disabled="busy"
          @click="update('pick_game_icon')"
        >
          <AppIcon :name="busy ? 'loader-circle' : 'folder'" />
          {{ t('gameAppearance.chooseImage') }}
        </button>
        <button
          v-if="game.icon_ref"
          type="button"
          class="text-button"
          :disabled="busy"
          @click="update('reset_game_icon')"
        >
          {{ t('gameAppearance.reset') }}
        </button>
      </div>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
    </div>
  </SettingsDisclosure>
</template>
<style scoped>
.game-appearance-panel{font-size:13px}

.appearance-controls{padding:16px 0 4px}.appearance-presets{display:flex;gap:8px;flex-wrap:wrap}
.appearance-preset{display:inline-flex;align-items:center;gap:7px;border:1px solid var(--line);border-radius:8px;padding:8px 10px;background:transparent;color:var(--ink);font:inherit;cursor:pointer}
.appearance-preset[aria-pressed="true"]{border-color:var(--accent);box-shadow:0 0 0 1px var(--accent)}
.appearance-preset:disabled{opacity:.55;cursor:default}.appearance-swatch{width:15px;height:15px;border-radius:4px;box-shadow:inset 0 0 0 1px #0002}
.appearance-actions{display:flex;align-items:center;gap:14px;flex-wrap:wrap;margin-top:14px}.appearance-actions .secondary-button{display:inline-flex;align-items:center;gap:7px}
.appearance-controls .settings-error{margin:12px 0 0}
</style>
