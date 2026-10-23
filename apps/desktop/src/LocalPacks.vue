<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from 'reka-ui';
import DiscoverPacks from './DiscoverPacks.vue';
import type { LocalPack, LocalPackKind } from './games';
const props = defineProps<{ gameId: string; kind: LocalPackKind; locked: boolean; revision?: number; installedOnly?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const label = computed(() => t(props.kind === 'resource_pack' ? 'localPacks.resources' : 'localPacks.shaders'));
const packs = ref<LocalPack[]>([]);
const query = ref('');
const reading = ref(false);
const operation = ref<'import' | 'remove' | null>(null);
const folderBusy = ref(false);
const error = ref('');
const readFailed = ref(false);
const deleting = ref<string | null>(null);
const tab = ref('installed');
const discoveryVisited = ref(false);
const discoveringBusy = ref(false);
const mutating = computed(() => operation.value !== null || discoveringBusy.value);
function discoveryBusy(value: boolean) { discoveringBusy.value = value; emit('busy', value); }
function installed(value: LocalPack[]) { ++request; packs.value = value; query.value = ''; deleting.value = null; error.value = ''; readFailed.value = false; reading.value = false; tab.value = 'installed'; }
watch(tab, (value) => { if (value === 'discover') discoveryVisited.value = true; });
const filtered = computed(() => {
  const search = query.value.trim().toLocaleLowerCase();
  return packs.value.filter((pack) => [pack.name, pack.file_name, pack.description].some((value) => value?.toLocaleLowerCase().includes(search)));
});
let request = 0;
function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const unit = bytes < 1024 ** 2 ? 'KiB' : bytes < 1024 ** 3 ? 'MiB' : 'GiB';
  const divisor = unit === 'KiB' ? 1024 : unit === 'MiB' ? 1024 ** 2 : 1024 ** 3;
  return `${new Intl.NumberFormat(locale.value, { maximumFractionDigits: 1 }).format(bytes / divisor)} ${unit}`;
}
async function load() {
  if (mutating.value) return;
  const current = ++request;
  reading.value = true; error.value = ''; readFailed.value = false;
  try {
    const value = await invoke<LocalPack[]>('list_game_packs', { id: props.gameId, kind: props.kind });
    if (current === request) { packs.value = value; if (!value.some((pack) => pack.file_name === deleting.value)) deleting.value = null; }
  } catch { if (current === request) { error.value = t('localPacks.readError', {kind: label.value}); readFailed.value = true; } }
  finally { if (current === request) reading.value = false; }
}
async function importPacks() {
  if (props.locked || mutating.value || reading.value) return;
  const current = ++request;
  operation.value = 'import'; error.value = ''; readFailed.value = false; emit('busy', true);
  try {
    const value = await invoke<LocalPack[] | null>('import_game_packs', { id: props.gameId, kind: props.kind });
    if (current === request && value) packs.value = value;
  } catch { if (current === request) error.value = t('localPacks.importError', {kind: label.value}); }
  finally { operation.value = null; emit('busy', false); }
}
async function remove(pack: LocalPack) {
  if (props.locked || mutating.value || reading.value) return;
  const current = ++request;
  operation.value = 'remove'; error.value = ''; readFailed.value = false; emit('busy', true);
  try {
    const value = await invoke<LocalPack[]>('remove_game_pack', { id: props.gameId, kind: props.kind, fileName: pack.file_name });
    if (current === request) { packs.value = value; deleting.value = null; }
  } catch { if (current === request) error.value = t('localPacks.removeError', {kind: label.value}); }
  finally { operation.value = null; emit('busy', false); }
}
async function openFolder() {
  if (folderBusy.value) return;
  folderBusy.value = true; error.value = ''; readFailed.value = false;
  try { await invoke('open_game_pack_directory', { id: props.gameId, kind: props.kind }); }
  catch { error.value = t('localPacks.folderError', {kind: label.value}); }
  finally { folderBusy.value = false; }
}
watch(() => props.locked, (locked) => { if (locked && !mutating.value) deleting.value = null; });
watch(() => props.revision, () => { deleting.value = null; tab.value = 'installed'; void load(); });
watch(() => [props.gameId, props.kind], () => { tab.value = 'installed'; discoveryVisited.value = false; packs.value = []; query.value = ''; deleting.value = null; void load(); }, { immediate: true });
onUnmounted(() => { ++request; });
</script>
<template>
  <div class="local-packs">
    <TabsRoot
      v-model="tab"
      :unmount-on-hide="false"
    >
      <TabsList
        v-if="!installedOnly"
        class="content-tabs local-pack-tabs"
        :aria-label="t('localPacks.tabs', {kind: label})"
      >
        <TabsTrigger
          value="installed"
          :disabled="mutating"
        >
          {{ t('localPacks.installed') }} <span class="count">{{ packs.length }}</span>
        </TabsTrigger>
        <TabsTrigger
          value="discover"
          :disabled="mutating"
        >
          {{ t('localPacks.discover', {kind: label}) }}
        </TabsTrigger>
      </TabsList>
      <component
        :is="installedOnly ? 'div' : TabsContent"
        value="installed"
        class="local-pack-installed"
      >
        <div class="content-heading">
          <div>
            <p
              v-if="installedOnly"
              class="local-packs-count"
            >
              {{ t('localPacks.installedCount', {count: packs.length}) }}
            </p><h2 v-else>
              {{ t('localPacks.local', {kind: label}) }} <span class="count">{{ packs.length }}</span>
            </h2><p class="setting-hint">
              {{ t(kind === 'resource_pack' ? 'localPacks.resourceHelp' : 'localPacks.shaderHelp') }}
            </p>
          </div>
          <div class="local-pack-heading-actions">
            <button
              class="secondary-button"
              :disabled="locked || mutating || reading"
              @click="importPacks"
            >
              {{ operation === 'import' ? t('localPacks.importing') : t('localPacks.import', {kind: label}) }}
            </button><button
              class="secondary-button"
              :disabled="folderBusy"
              :aria-label="t('localPacks.openFolder', {kind: label})"
              @click="openFolder"
            >
              <AppIcon name="folder" />
            </button>
          </div>
        </div>
        <div class="content-toolbar">
          <input
            v-model="query"
            type="search"
            :aria-label="t('localPacks.search', {kind: label})"
            :placeholder="t('localPacks.searchPlaceholder')"
            :disabled="mutating"
          ><button
            class="secondary-button"
            :disabled="reading || mutating"
            :aria-label="t('localPacks.refresh')"
            @click="load"
          >
            <AppIcon :name="reading ? 'loader-circle' : 'refresh-cw'" />
          </button>
        </div>
        <p
          v-if="error"
          class="settings-error"
          role="alert"
        >
          {{ error }} <button
            v-if="readFailed"
            class="secondary-button"
            @click="load"
          >
            {{ t('localPacks.retry') }}
          </button>
        </p>
        <p
          v-if="reading && !packs.length"
          class="content-empty"
          role="status"
        >
          {{ t('localPacks.reading', {kind: label}) }}
        </p>
        <ul
          v-else-if="filtered.length"
          class="local-pack-list"
          :aria-label="t('localPacks.local', {kind: label})"
        >
          <li
            v-for="pack in filtered"
            :key="pack.file_name"
            class="local-pack-row"
          >
            <div class="local-pack-main">
              <span
                class="local-pack-icon"
                aria-hidden="true"
              ><AppIcon
                name="folder"
                :size="20"
              /></span><div class="mod-info">
                <h3>{{ pack.name }}</h3><p
                  v-if="pack.description"
                  class="mod-description"
                >
                  {{ pack.description }}
                </p><p class="mod-meta">
                  {{ sizeLabel(pack.size_bytes) }}
                </p><p class="mod-filename">
                  {{ pack.file_name }}
                </p>
              </div><button
                v-if="deleting !== pack.file_name"
                class="secondary-button"
                :aria-label="t('localPacks.removeNamed', {name: pack.name})"
                :disabled="locked || mutating || reading"
                @click="deleting = pack.file_name"
              >
                <AppIcon name="trash" />
              </button>
            </div>
            <div
              v-if="deleting === pack.file_name"
              class="local-pack-confirm"
            >
              <p>{{ t('localPacks.confirm', {name: pack.name}) }}</p><div class="profile-form-actions">
                <button
                  class="secondary-button danger-text"
                  :disabled="locked || mutating || reading"
                  @click="remove(pack)"
                >
                  {{ t(operation === 'remove' ? 'localPacks.removing' : 'localPacks.confirmRemove') }}
                </button><button
                  class="secondary-button"
                  :disabled="mutating"
                  @click="deleting = null"
                >
                  {{ t('localPacks.cancel') }}
                </button>
              </div>
            </div>
          </li>
        </ul>
        <p
          v-else-if="!error"
          class="content-empty"
        >
          {{ t(query.trim() ? 'localPacks.noMatches' : 'localPacks.empty', {kind: label}) }}
        </p>
      </component>
      <TabsContent
        v-if="!installedOnly"
        value="discover"
      >
        <DiscoverPacks
          v-if="discoveryVisited"
          :game-id="gameId"
          :kind="kind"
          :revision="revision"
          :locked="locked || operation !== null"
          @busy="discoveryBusy"
          @installed="installed"
        />
      </TabsContent>
    </TabsRoot>
  </div>
</template>
<style scoped>
.local-packs .content-heading{display:flex;justify-content:space-between;align-items:center;gap:14px;margin:18px 0 12px}.local-packs-count{margin:0;font-size:12px;color:var(--muted)}.local-packs .setting-hint{margin:5px 0 0}.local-pack-heading-actions{display:flex;gap:8px;flex-shrink:0}.local-pack-heading-actions button,.local-packs .content-toolbar button,.local-pack-main>button{display:inline-flex;align-items:center;justify-content:center;gap:7px;min-height:36px;padding:8px 11px}.local-packs .content-toolbar{margin-top:14px}.local-packs .mod-info{min-width:0}.local-packs .local-pack-row{overflow-wrap:anywhere}@media(max-width:560px){.local-packs .content-heading{align-items:flex-start}.local-pack-heading-actions{gap:6px}.local-pack-heading-actions button{font-size:12px;padding:8px 10px}}
</style>
