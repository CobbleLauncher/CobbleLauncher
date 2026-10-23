<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
export type SavedServer = { id: string; name: string; address: string; can_connect: boolean };
type SavedServers = { items: SavedServer[]; direct_connect: boolean };
const props = defineProps<{ gameId: string; locked: boolean; running: boolean }>();
const emit = defineEmits<{ play: [serverId: string] }>();
const servers = ref<SavedServer[]>([]);
const supported = ref(true);
const reading = ref(false);
const error = ref('');
const copied = ref<string | null>(null);
const query = ref('');
const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase();
  return servers.value.filter(server => `${server.name} ${server.address}`.toLocaleLowerCase().includes(needle));
});
let request = 0;
async function load() {
  const current = ++request;
  reading.value = true; error.value = '';
  try {
    const value = await invoke<SavedServers>('list_game_servers', { gameId: props.gameId });
    if (current === request) { servers.value = value.items; supported.value = value.direct_connect; }
  } catch (cause) { if (current === request) error.value = String(cause); }
  finally { if (current === request) reading.value = false; }
}
async function copy(server: SavedServer) {
  try { await writeText(server.address); copied.value = server.id; error.value = ''; }
  catch { error.value = t('gameServers.copyError'); }
}
function join(server: SavedServer) {
  if (!props.locked && !props.running && server.can_connect) emit('play', server.id);
}
watch(() => props.gameId, () => { servers.value = []; query.value = ''; copied.value = null; void load(); }, { immediate: true });
onUnmounted(() => { ++request; });
</script>
<template>
  <section class="game-servers-panel">
    <div class="content-heading">
      <h2>{{ t('gameServers.title') }} <span class="count">{{ servers.length }}</span></h2>
      <button
        class="text-button"
        :disabled="reading"
        @click="load"
      >
        <AppIcon name="refresh-cw" />{{ t('gameServers.refresh') }}
      </button>
    </div>
    <div
      v-if="servers.length > 5"
      class="content-toolbar"
    >
      <input
        v-model="query"
        type="search"
        :aria-label="t('gameServers.search')"
        :placeholder="t('gameServers.search')"
      >
    </div>
    <p
      v-if="running && servers.length"
      class="setting-hint"
    >
      {{ t('gameServers.running') }}
    </p>
    <p
      v-else-if="!supported && servers.length"
      class="setting-hint"
    >
      {{ t('gameServers.unsupported') }}
    </p>
    <p
      v-if="error"
      class="inline-error"
      role="alert"
    >
      {{ error }}
    </p>
    <LoadingIndicator
      v-if="reading && !servers.length"
      :label="t('gameServers.loading')"
    />
    <ul
      v-else-if="filtered.length"
      class="saved-server-list"
      :aria-label="t('gameServers.title')"
    >
      <li
        v-for="server in filtered"
        :key="server.id"
        class="saved-server-row"
      >
        <AppIcon
          name="network"
          :size="22"
        />
        <div class="saved-server-info">
          <h3>{{ server.name || server.address }}</h3><p v-if="server.name">
            {{ server.address }}
          </p>
        </div>
        <div class="saved-server-actions">
          <button
            class="text-button"
            :aria-label="t('gameServers.copyNamed', { name: server.name || server.address })"
            @click="copy(server)"
          >
            <AppIcon :name="copied === server.id ? 'check' : 'copy'" />{{ copied === server.id ? t('gameServers.copied') : t('gameServers.copy') }}
          </button>
          <button
            v-if="server.can_connect && !running"
            class="secondary-button"
            :disabled="locked"
            @click="join(server)"
          >
            <AppIcon name="play" />{{ t('gameServers.join') }}
          </button>
        </div>
      </li>
    </ul>
    <p
      v-else-if="!reading && !error"
      class="content-empty"
    >
      {{ query ? t('gameServers.noMatches') : t('gameServers.empty') }}
    </p>
  </section>
</template>
<style scoped>
.saved-server-list { padding: 0; margin: 12px 0; list-style: none; }
.saved-server-row { display: flex; align-items: center; gap: 14px; padding: 14px 0; border-bottom: 1px solid var(--ui-border); }
.saved-server-row > .app-icon { color: var(--ui-text-muted); flex: none; }
.saved-server-info { flex: 1; min-width: 0; }
.saved-server-info h3 { margin: 0; font-size: 14px; font-weight: 600; overflow-wrap: anywhere; }
.saved-server-info p { margin: 4px 0 0; color: var(--ui-text-muted); font-size: 12px; overflow-wrap: anywhere; }
.saved-server-actions { display: flex; gap: 8px; flex: none; }
@media (max-width: 600px) { .saved-server-row { flex-wrap: wrap; gap: 10px; } .saved-server-actions { margin-left: 32px; } }
</style>
