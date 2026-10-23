<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import { PACK_UPDATE_GROUPS, packChangePath, packChangeMatches, packUpdateGroups, type PackUpdateContext, type PackUpdatePreview, type PackUpdateGroup } from './pack-update';
const props = defineProps<{gameId: string}>();
const emit = defineEmits<{busy: [value: boolean]}>();
const context = ref<PackUpdateContext | null>(null); const contextError = ref(false); const contextLoading = ref(false);
const visible = ref(false); const pending = ref(false); const error = ref(''); const preview = ref<PackUpdatePreview | null>(null);
const group = ref<PackUpdateGroup>('version_changed'); const search = ref(''); const page = ref(0); const pageSize = 50;
const supported = computed(() => context.value?.supported === true);
const groups = computed(() => preview.value ? packUpdateGroups(preview.value) : null);
const matched = computed(() => (groups.value?.[group.value] ?? []).filter(change => packChangeMatches(change, search.value)));
const rows = computed(() => matched.value.slice(page.value * pageSize, (page.value + 1) * pageSize));
let disposed = false; let request = 0;
watch(pending, value => emit('busy', value), {flush:'sync'});
watch([group, search], () => {page.value = 0;});
async function loadContext() {
  const current = ++request; contextLoading.value = true; contextError.value = false;
  try { const value = await invoke<PackUpdateContext>('get_game_pack_update_context', {gameId:props.gameId}); if (!disposed && current === request) context.value = value; }
  catch { if (!disposed && current === request) contextError.value = true; }
  finally { if (!disposed && current === request) contextLoading.value = false; }
}
function errorMessage(cause: unknown) {
  const code = String(cause); const keys = {PACK_UPDATE_UNSUPPORTED:'unsupported',PACK_UPDATE_SOURCE_CHANGED:'sourceChanged',PACK_UPDATE_ARCHIVE_TOO_LARGE:'tooLarge',PACK_UPDATE_INVALID_ARCHIVE:'invalidArchive',PACK_UPDATE_IO:'failed',PACK_UPDATE_TOO_MANY_FILES:'tooManyFiles'} as const;
  return t(`packUpdate.${keys[code as keyof typeof keys] ?? 'failed'}`);
}
async function choose() {
  if (!supported.value || pending.value) return;
  const current = request; pending.value = true; error.value = ''; visible.value = true;
  try {
    const result = await invoke<PackUpdatePreview | null>('preview_game_pack_update', {gameId:props.gameId});
    if (disposed || current !== request) return;
    if (!result) { if (!preview.value) visible.value = false; return; }
    preview.value = result; search.value = ''; page.value = 0;
    const all = packUpdateGroups(result); group.value = PACK_UPDATE_GROUPS.find(id => id !== 'unchanged' && all[id].length) ?? 'version_changed';
  } catch (cause) { if (!disposed && current === request) {preview.value = null; error.value = errorMessage(cause);} }
  finally { if (!disposed && current === request) pending.value = false; }
}
watch(() => props.gameId, () => { context.value = null; preview.value = null; visible.value = false; error.value = ''; pending.value = false; void loadContext(); }, {immediate:true});
onUnmounted(() => {disposed = true; ++request; if (pending.value) emit('busy',false);});
defineExpose({choose,supported,pending});
</script>
<template>
  <div
    v-if="contextError"
    class="pack-update-context-error"
  >
    <p
      class="settings-error"
      role="alert"
    >
      {{ t('packUpdate.contextFailed') }}
    </p><button
      class="text-button"
      :disabled="contextLoading"
      @click="loadContext"
    >
      {{ t('plugins.retry') }}
    </button>
  </div>
  <section
    v-if="visible"
    class="pack-update-panel"
    :aria-busy="pending"
  >
    <div class="pack-update-heading">
      <h2>{{ t('packUpdate.title') }}</h2><div>
        <button
          class="text-button"
          :disabled="pending"
          @click="choose"
        >
          {{ t('packUpdate.choose') }}
        </button><AppIconButton
          icon="x"
          :label="t('packUpdate.close')"
          :disabled="pending"
          @click="visible = false"
        />
      </div>
    </div>
    <LoadingIndicator
      v-if="pending"
      :label="t('packUpdate.loading')"
      compact
    />
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <template v-if="preview && !pending">
      <dl class="pack-update-versions">
        <div><dt>{{ t('packUpdate.installed') }}</dt><dd>{{ context?.name }}<span v-if="context?.name"> · </span>{{ preview.current_version }}</dd></div><div><dt>{{ t('packUpdate.target') }}</dt><dd>{{ preview.target_name }} · {{ preview.target_version }}</dd></div>
      </dl>
      <p class="setting-hint">
        {{ t('packUpdate.reviewOnly') }}
      </p>
      <p
        v-if="preview.identity_relation === 'unknown'"
        class="pack-update-warning"
      >
        {{ t('packUpdate.checkIdentity') }}
      </p>
      <p
        v-if="preview.runtime_changed"
        class="pack-update-warning"
      >
        {{ t('packUpdate.runtimeChanged') }}
      </p>
      <p
        v-if="preview.excluded_optional_files"
        class="setting-hint"
      >
        {{ t('packUpdate.optionalExcluded', {count:preview.excluded_optional_files}) }}
      </p>
      <div
        class="pack-update-groups"
        role="group"
        :aria-label="t('packUpdate.groups')"
      >
        <button
          v-for="id in PACK_UPDATE_GROUPS"
          :key="id"
          class="text-button"
          :aria-pressed="group === id"
          @click="group = id"
        >
          {{ t(`packUpdate.${id}`) }} <span>{{ groups?.[id].length ?? 0 }}</span>
        </button>
      </div>
      <label class="pack-update-search"><AppIcon name="search" /><input
        v-model="search"
        type="search"
        :aria-label="t('packUpdate.search')"
        :placeholder="t('packUpdate.search')"
      ></label>
      <p
        v-if="!matched.length"
        class="setting-hint"
      >
        {{ t(search.trim() ? 'packUpdate.noMatches' : 'packUpdate.empty') }}
      </p>
      <ul class="pack-update-files">
        <li
          v-for="(change,index) in rows"
          :key="`${page}:${index}:${packChangePath(change)}`"
        >
          <div class="pack-update-path">
            <strong>{{ packChangePath(change) }}</strong><span
              v-if="change.preserve"
              class="pack-update-badge"
            >{{ t('packUpdate.keepLocal') }}</span>
          </div><p
            v-if="change.before && change.after && change.before.path !== change.after.path"
            class="setting-hint"
          >
            {{ change.before.path }} → {{ change.after.path }}
          </p><p
            v-if="change.before?.version_id || change.after?.version_id"
            class="setting-hint"
          >
            {{ change.before?.version_id ?? '—' }} → {{ change.after?.version_id ?? '—' }}
          </p><span
            v-if="change.local_state !== 'clean'"
            class="setting-hint"
          >{{ t(`packUpdate.local_${change.local_state}`) }}</span>
        </li>
      </ul>
      <nav
        v-if="matched.length > pageSize"
        class="pack-update-pager"
        :aria-label="t('packUpdate.pages')"
      >
        <button
          class="text-button"
          :disabled="page === 0"
          @click="page--"
        >
          {{ t('packUpdate.previous') }}
        </button><span>{{ page * pageSize + 1 }}–{{ Math.min((page+1)*pageSize,matched.length) }} / {{ matched.length }}</span><button
          class="text-button"
          :disabled="(page+1)*pageSize >= matched.length"
          @click="page++"
        >
          {{ t('packUpdate.next') }}
        </button>
      </nav>
    </template>
  </section>
</template>
<style scoped>
.pack-update-panel{margin:20px 0;padding:20px;border:1px solid var(--line);border-radius:12px;background:var(--panel);min-width:0}.pack-update-heading,.pack-update-heading>div,.pack-update-path,.pack-update-pager{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.pack-update-heading{justify-content:space-between}.pack-update-heading h2{font-size:16px;margin:0}.pack-update-versions{display:flex;gap:28px;flex-wrap:wrap;margin:18px 0}.pack-update-versions dt{font-size:11px;color:var(--muted)}.pack-update-versions dd{margin:5px 0;font-size:14px;overflow-wrap:anywhere}.pack-update-warning{font-size:12px;color:var(--ink);margin:12px 0}.pack-update-groups{display:flex;flex-wrap:wrap;gap:7px;margin:18px 0 12px}.pack-update-groups [aria-pressed=true]{border-color:var(--accent);background:var(--tint)}.pack-update-groups span{margin-left:5px;color:var(--muted)}.pack-update-search{display:flex;align-items:center;gap:8px;border:1px solid var(--line);border-radius:8px;padding:0 10px}.pack-update-search svg{width:15px;height:15px;flex:none}.pack-update-search input{min-width:0;width:100%;min-height:40px;background:transparent;color:var(--ink);font:inherit;border:0;padding:8px 0}.pack-update-search input::-webkit-search-decoration,.pack-update-search input::-webkit-search-cancel-button{appearance:none}.pack-update-files{list-style:none;padding:0;margin:12px 0}.pack-update-files li{border-bottom:1px solid var(--line);padding:12px 0;overflow-wrap:anywhere}.pack-update-path strong{font-size:12px;font-weight:500}.pack-update-badge{padding:3px 7px;border:1px solid var(--line);border-radius:6px;font-size:10px;color:var(--muted)}.pack-update-files p{margin:5px 0}.pack-update-files .setting-hint{font-size:11px}.pack-update-pager{justify-content:flex-end;font-size:11px;color:var(--muted)}.pack-update-context-error{display:flex;align-items:center;gap:12px}@media(max-width:600px){.pack-update-panel{padding:14px}.pack-update-versions{gap:16px}.pack-update-heading>div{gap:5px}}
</style>
