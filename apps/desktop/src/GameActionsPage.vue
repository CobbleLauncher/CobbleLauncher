<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { gameLoaderLabel, type Game, type GameAction } from './games';
const props = defineProps<{ game: Game; action: GameAction; starting: boolean }>();
const emit = defineEmits<{ copied: [game: Game]; removed: [id: string]; busy: [value: boolean]; cancel: [] }>();
const copyName = ref(`${props.game.name.slice(0, 77)} 副本`);
const includeSaves = ref(false);
const removeMode = ref<'keep' | 'delete'>('keep');
const busy = ref(false);
const error = ref('');
const exported = ref(false);
const locked = computed(() => props.starting || props.game.status === 'installing' || props.game.content_installing || props.game.launch_state === 'preparing' || props.game.launch_state === 'running');
const unavailable = computed(() => busy.value || locked.value);
const buttonLabel = computed(() => busy.value ? props.action === 'copy' ? '复制中…' : props.action === 'export' ? '导出中…' : '移除中…' : props.action === 'copy' ? '创建副本' : props.action === 'export' ? '选择位置并导出' : removeMode.value === 'delete' ? '删除游戏和文件' : '从游戏库移除');
async function submit() {
  if (unavailable.value) return;
  error.value = ''; exported.value = false;
  const name = copyName.value.trim();
  if (props.action === 'copy' && (!name || name.length > 80)) { error.value = '副本名称需为 1–80 个字符'; return; }
  busy.value = true; emit('busy', true);
  try {
    if (props.action === 'copy') emit('copied', await invoke<Game>('copy_game', { id: props.game.id, name, includeSaves: includeSaves.value }));
    else if (props.action === 'export') exported.value = await invoke<boolean>('export_game', { id: props.game.id, includeSaves: includeSaves.value });
    else { await invoke('remove_game', { id: props.game.id, deleteFiles: removeMode.value === 'delete' }); emit('removed', props.game.id); }
  } catch (cause) { error.value = `无法${props.action === 'copy' ? '复制游戏' : props.action === 'export' ? '导出整合包' : '移除游戏'}：${String(cause)}`; }
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
      <div><h2>{{ game.name }}</h2><p>Minecraft {{ game.version }} · {{ gameLoaderLabel(game) }}</p></div>
    </div>
    <form
      class="game-action-form"
      novalidate
      @submit.prevent="submit"
    >
      <template v-if="action === 'copy'">
        <p class="action-description">
          复制游戏、Mods 和配置到独立目录。
        </p>
        <div class="form-field">
          <label for="copy-game-name">副本名称</label><input
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
        ><span>包含存档<small>把当前世界一同复制到副本。</small></span></label>
      </template>
      <template v-else-if="action === 'export'">
        <p class="action-description">
          导出为 .mrpack 整合包，包含 Mods 和配置。游戏本体、资源缓存和玩家账号不会导出。
        </p>
        <label class="game-action-checkbox"><input
          v-model="includeSaves"
          type="checkbox"
          :disabled="unavailable"
        ><span>包含存档<small>选中后，整合包也会包含你的世界。</small></span></label>
      </template>
      <template v-else>
        <RadioGroupRoot
          v-model="removeMode"
          class="game-remove-options"
          aria-label="移除方式"
          :disabled="unavailable"
        >
          <RadioGroupItem
            value="keep"
            class="game-remove-option"
            aria-label="仅从游戏库移除"
          >
            <span
              class="remove-radio"
              aria-hidden="true"
            /><span><strong>仅从游戏库移除</strong><small>游戏文件和存档保留在原目录。</small></span>
          </RadioGroupItem>
          <RadioGroupItem
            value="delete"
            class="game-remove-option"
            aria-label="连同文件删除"
          >
            <span
              class="remove-radio"
              aria-hidden="true"
            /><span><strong>连同文件删除</strong><small>删除整个游戏目录，包含 Mods、配置和存档。</small></span>
          </RadioGroupItem>
        </RadioGroupRoot>
        <div
          class="game-removal-path"
          :class="{ 'delete-selected': removeMode === 'delete' }"
        >
          <p>{{ removeMode === 'delete' ? '将删除以下目录及其中所有文件，此操作无法撤销。' : '保留文件的目录' }}</p><p class="java-path">
            {{ game.directory }}
          </p>
        </div>
      </template>
      <p
        v-if="locked"
        class="setting-hint"
        role="status"
      >
        请先关闭游戏并等待当前任务完成。
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
        整合包已导出
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
          取消
        </button>
      </div>
    </form>
  </div>
</template>
