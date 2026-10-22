<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from 'reka-ui';
import DiscoverPacks from './DiscoverPacks.vue';
import type { LocalPack, LocalPackKind } from './games';
const props = defineProps<{ gameId: string; kind: LocalPackKind; locked: boolean; revision?: number }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const label = computed(() => props.kind === 'resource_pack' ? '资源包' : '光影包');
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
  return `${new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 1 }).format(bytes / divisor)} ${unit}`;
}
async function load() {
  if (mutating.value) return;
  const current = ++request;
  reading.value = true; error.value = ''; readFailed.value = false;
  try {
    const value = await invoke<LocalPack[]>('list_game_packs', { id: props.gameId, kind: props.kind });
    if (current === request) { packs.value = value; if (!value.some((pack) => pack.file_name === deleting.value)) deleting.value = null; }
  } catch (cause) { if (current === request) { error.value = `无法读取${label.value}：${String(cause)}`; readFailed.value = true; } }
  finally { if (current === request) reading.value = false; }
}
async function importPacks() {
  if (props.locked || mutating.value || reading.value) return;
  const current = ++request;
  operation.value = 'import'; error.value = ''; readFailed.value = false; emit('busy', true);
  try {
    const value = await invoke<LocalPack[] | null>('import_game_packs', { id: props.gameId, kind: props.kind });
    if (current === request && value) packs.value = value;
  } catch (cause) { if (current === request) error.value = `导入${label.value}未完成：${String(cause)}`; }
  finally { operation.value = null; emit('busy', false); }
}
async function remove(pack: LocalPack) {
  if (props.locked || mutating.value || reading.value) return;
  const current = ++request;
  operation.value = 'remove'; error.value = ''; readFailed.value = false; emit('busy', true);
  try {
    const value = await invoke<LocalPack[]>('remove_game_pack', { id: props.gameId, kind: props.kind, fileName: pack.file_name });
    if (current === request) { packs.value = value; deleting.value = null; }
  } catch (cause) { if (current === request) error.value = `无法移除${label.value}：${String(cause)}`; }
  finally { operation.value = null; emit('busy', false); }
}
async function openFolder() {
  if (folderBusy.value) return;
  folderBusy.value = true; error.value = ''; readFailed.value = false;
  try { await invoke('open_game_pack_directory', { id: props.gameId, kind: props.kind }); }
  catch (cause) { error.value = `无法打开${label.value}文件夹：${String(cause)}`; }
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
        class="content-tabs local-pack-tabs"
        :aria-label="`${label}内容`"
      >
        <TabsTrigger
          value="installed"
          :disabled="mutating"
        >
          已安装 <span class="count">{{ packs.length }}</span>
        </TabsTrigger>
        <TabsTrigger
          value="discover"
          :disabled="mutating"
        >
          发现{{ label }}
        </TabsTrigger>
      </TabsList>
      <TabsContent
        value="installed"
        class="local-pack-installed"
      >
        <div class="content-heading">
          <div>
            <h2>本地{{ label }} <span class="count">{{ packs.length }}</span></h2><p class="setting-hint">
              {{ kind === 'resource_pack' ? '在游戏内选择资源包。' : '需要支持光影的游戏环境。' }}
            </p>
          </div><button
            class="secondary-button"
            :disabled="locked || mutating || reading"
            @click="importPacks"
          >
            {{ operation === 'import' ? '导入中…' : `导入${label}` }}
          </button>
        </div>
        <div class="pack-folder-actions">
          <button
            class="text-button"
            :disabled="folderBusy"
            @click="openFolder"
          >
            打开{{ label }}文件夹
          </button>
        </div>
        <div class="content-toolbar">
          <input
            v-model="query"
            type="search"
            :aria-label="`搜索${label}`"
            placeholder="搜索名称或描述"
            :disabled="mutating"
          ><button
            class="text-button"
            :disabled="reading || mutating"
            @click="load"
          >
            {{ reading ? '读取中…' : '刷新' }}
          </button>
        </div>
        <p
          v-if="error"
          class="settings-error"
          role="alert"
        >
          {{ error }} <button
            v-if="readFailed"
            class="text-button"
            @click="load"
          >
            重试读取
          </button>
        </p>
        <p
          v-if="reading && !packs.length"
          class="content-empty"
          role="status"
        >
          正在读取{{ label }}…
        </p>
        <ul
          v-else-if="filtered.length"
          class="local-pack-list"
          :aria-label="`本地${label}`"
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
              ><svg viewBox="0 0 24 24"><path
                v-if="kind === 'resource_pack'"
                d="m4 7 8-4 8 4-8 4zm0 0v10l8 4 8-4V7M12 11v10"
              /><path
                v-else
                d="m12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5z"
              /></svg></span><div class="mod-info">
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
                class="text-button"
                :aria-label="`移除 ${pack.name}`"
                :disabled="locked || mutating || reading"
                @click="deleting = pack.file_name"
              >
                移除
              </button>
            </div><div
              v-if="deleting === pack.file_name"
              class="local-pack-confirm"
            >
              <p>移除 {{ pack.name }}？只删除这个 ZIP 文件。</p><div class="profile-form-actions">
                <button
                  class="secondary-button danger-text"
                  :disabled="locked || mutating || reading"
                  @click="remove(pack)"
                >
                  {{ operation === 'remove' ? '移除中…' : '确认移除' }}
                </button><button
                  class="text-button"
                  :disabled="mutating"
                  @click="deleting = null"
                >
                  取消
                </button>
              </div>
            </div>
          </li>
        </ul>
        <p
          v-else-if="!error"
          class="content-empty"
        >
          {{ query.trim() ? `没有找到匹配的${label}。` : `还没有${label}，导入本地 ZIP 文件开始。` }}
        </p>
      </TabsContent>
      <TabsContent value="discover">
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
