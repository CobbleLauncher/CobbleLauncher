<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Keyboard, X } from '@lucide/vue';
import { t } from './i18n';

type InstalledPlugin = {
  enabled: boolean;
  problem?: string;
  manifest: {
    id: string;
    frontend?: unknown;
    permissions?: { permission: string }[];
  };
};

const props = defineProps<{ gameId: string }>();
const emit = defineEmits<{ reuse: [targetGameId: string]; dismiss: [] }>();
const available = ref(false);

onMounted(async () => {
  try {
    const plugins = await invoke<InstalledPlugin[]>('list_plugins');
    available.value = plugins.some(plugin => plugin.enabled && !plugin.problem
      && plugin.manifest.id === 'config-doctor'
      && !!plugin.manifest.frontend
      && plugin.manifest.permissions?.some(item => item.permission === 'launcher:game-keybindings/read')
      && plugin.manifest.permissions?.some(item => item.permission === 'launcher:game-keybindings/write'));
  } catch {
    available.value = false;
  }
});
</script>

<template>
  <div
    v-if="available"
    class="created-game-preferences"
  >
    <button
      class="created-game-preferences-action"
      @click="emit('reuse', props.gameId)"
    >
      <Keyboard
        :size="16"
        aria-hidden="true"
      />
      {{ t('createdGamePreferences.useKeys') }}
    </button>
    <button
      class="created-game-preferences-dismiss"
      :aria-label="t('createdGamePreferences.dismiss')"
      @click="emit('dismiss')"
    >
      <X
        :size="15"
        aria-hidden="true"
      />
    </button>
  </div>
</template>

<style scoped>
.created-game-preferences{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-top:14px;padding:10px 12px;border:1px solid var(--line);border-radius:10px;background:var(--panel)}
.created-game-preferences-action,.created-game-preferences-dismiss{display:flex;align-items:center;justify-content:center;gap:8px;border:0;background:transparent;color:var(--ink)}
.created-game-preferences-action{min-height:32px;font-size:14px;font-weight:550}
.created-game-preferences-action:hover,.created-game-preferences-dismiss:hover{color:var(--accent)}
.created-game-preferences-dismiss{width:30px;height:30px;border-radius:7px;color:var(--muted)}
</style>
