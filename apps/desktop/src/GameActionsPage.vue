<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { loaderLabel, type Game, type GameAction } from './games';
import { t } from './i18n';
const props = defineProps<{ game: Game; action: GameAction; starting: boolean }>();
const emit = defineEmits<{ copied: [game: Game]; removed: [id: string]; busy: [value: boolean]; cancel: [] }>();
const copySuffix = t('gameActions.copySuffix');
const copyName = ref(`${props.game.name.slice(0, 80 - copySuffix.length)}${copySuffix}`);
const includeSaves = ref(false);
const removeMode = ref<'keep' | 'delete'>('keep');
const busy = ref(false);
const error = ref('');
const exported = ref(false);
const loader = computed(() => props.game.loader_version ? `${loaderLabel(props.game.loader_kind ?? 'fabric')} ${props.game.loader_version}` : t('gameActions.vanilla'));
const gameContext = computed(() => t('gameActions.gameContext', { version: props.game.version, loader: loader.value }));
const locked = computed(() => props.starting || props.game.status === 'installing' || props.game.content_installing || props.game.launch_state === 'preparing' || props.game.launch_state === 'running');
const unavailable = computed(() => busy.value || locked.value);
const buttonLabel = computed(() => busy.value
  ? props.action === 'copy' ? t('gameActions.copyBusy') : props.action === 'export' ? t('gameActions.exportBusy') : t('gameActions.removeBusy')
  : props.action === 'copy' ? t('gameActions.copyButton') : props.action === 'export' ? t('gameActions.exportButton') : removeMode.value === 'delete' ? t('gameActions.deleteButton') : t('gameActions.removeButton'));
async function submit() {
  if (unavailable.value) return;
  error.value = ''; exported.value = false;
  const name = copyName.value.trim();
  if (props.action === 'copy' && (!name || name.length > 80)) { error.value = t('gameActions.copyNameInvalid'); return; }
  busy.value = true; emit('busy', true);
  try {
    if (props.action === 'copy') emit('copied', await invoke<Game>('copy_game', { id: props.game.id, name, includeSaves: includeSaves.value }));
    else if (props.action === 'export') exported.value = await invoke<boolean>('export_game', { id: props.game.id, includeSaves: includeSaves.value });
    else { await invoke('remove_game', { id: props.game.id, deleteFiles: removeMode.value === 'delete' }); emit('removed', props.game.id); }
  } catch (cause) {
    const message = props.action === 'copy' ? 'gameActions.copyError' : props.action === 'export' ? 'gameActions.exportError' : 'gameActions.removeError';
    error.value = t(message, { cause: String(cause) });
  }
  finally { busy.value = false; emit('busy', false); }
}
</script>
<template>
  <div class="page-content game-action-page">
    <div class="game-action-source">
      <span
        class="game-action-icon"
        aria-hidden="true"
      ><svg viewBox="0 0 24 24"><g v-if="action === 'copy'"><path d="M8 8h12v12H8zM4 16V4h12" /></g><g v-else-if="action === 'export'"><path d="M12 15V3m-4 4 4-4 4 4M5 13v7h14v-7" /></g><g v-else><path d="M5 7h14M9 7V4h6v3M7 7l1 13h8l1-13M10 10v7m4-7v7" /></g></svg></span>
      <div><h2>{{ game.name }}</h2><p>{{ gameContext }}</p></div>
    </div>
    <form
      class="game-action-form"
      novalidate
      @submit.prevent="submit"
    >
      <template v-if="action === 'copy'">
        <p class="action-description">
          {{ t('gameActions.copyDescription') }}
        </p>
        <div class="form-field">
          <label for="copy-game-name">{{ t('gameActions.copyName') }}</label><input
            id="copy-game-name"
            v-model="copyName"
            maxlength="80"
            autocomplete="off"
            :disabled="unavailable"
          >
        </div>
        <label class="game-action-checkbox"><input
          v-model="includeSaves"
          type="checkbox"
          :disabled="unavailable"
        ><span>{{ t('gameActions.includeSaves') }}<small>{{ t('gameActions.copySavesHelp') }}</small></span></label>
      </template>
      <template v-else-if="action === 'export'">
        <p class="action-description">
          {{ t('gameActions.exportDescription') }}
        </p>
        <label class="game-action-checkbox"><input
          v-model="includeSaves"
          type="checkbox"
          :disabled="unavailable"
        ><span>{{ t('gameActions.includeSaves') }}<small>{{ t('gameActions.exportSavesHelp') }}</small></span></label>
      </template>
      <template v-else>
        <RadioGroupRoot
          v-model="removeMode"
          class="game-remove-options"
          :aria-label="t('gameActions.removeMode')"
          :disabled="unavailable"
        >
          <RadioGroupItem
            value="keep"
            class="game-remove-option"
            :aria-label="t('gameActions.keepOption')"
          >
            <span
              class="remove-radio"
              aria-hidden="true"
            /><span><strong>{{ t('gameActions.keepOption') }}</strong><small>{{ t('gameActions.keepHelp') }}</small></span>
          </RadioGroupItem>
          <RadioGroupItem
            value="delete"
            class="game-remove-option"
            :aria-label="t('gameActions.deleteOption')"
          >
            <span
              class="remove-radio"
              aria-hidden="true"
            /><span><strong>{{ t('gameActions.deleteOption') }}</strong><small>{{ t('gameActions.deleteHelp') }}</small></span>
          </RadioGroupItem>
        </RadioGroupRoot>
        <div
          class="game-removal-path"
          :class="{ 'delete-selected': removeMode === 'delete' }"
        >
          <p>{{ removeMode === 'delete' ? t('gameActions.deleteWarning') : t('gameActions.keepPath') }}</p><p class="java-path">
            {{ game.directory }}
          </p>
        </div>
      </template>
      <p
        v-if="locked"
        class="setting-hint"
        role="status"
      >
        {{ t('gameActions.lockedWarning') }}
      </p>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <p
        v-if="exported"
        class="game-action-result"
        role="status"
      >
        {{ t('gameActions.exported') }}
      </p>
      <div class="profile-form-actions">
        <button
          class="primary-button"
          :class="{ 'destructive-button': action === 'remove' && removeMode === 'delete' }"
          type="submit"
          :disabled="unavailable"
        >
          <span
            v-if="busy"
            class="busy-spinner"
            aria-hidden="true"
          />{{ buttonLabel }}
        </button><button
          class="text-button"
          type="button"
          :disabled="busy"
          @click="$emit('cancel')"
        >
          {{ t('gameActions.cancel') }}
        </button>
      </div>
    </form>
  </div>
</template>
