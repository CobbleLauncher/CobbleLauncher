<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale } from './i18n';
import SelectField from './ui/SelectField.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import { changeIgnoredVersion, cloneModUpdatePolicy, emptyModUpdatePolicy, projectPolicyKey, versionPolicyCopy, type ModUpdateChannel, type ModUpdatePolicy } from './mod-version-policy';
const props = defineProps<{ gameId: string; source?: 'modrinth' | 'curseforge'; hideGameChannel?: boolean; projectId?: string; selectedVersion?: { version_id: string; version_number: string }; disabled?: boolean }>();
const emit = defineEmits<{ changed: []; busy: [value: boolean] }>();
const copy = computed(() => versionPolicyCopy[locale.value]);
const policy = ref<ModUpdatePolicy>(emptyModUpdatePolicy());
const busy = ref(false), loaded = ref(false), error = ref('');
const key = computed(() => props.projectId ? projectPolicyKey(props.projectId, props.source ?? 'modrinth') : undefined);
const title = computed(() => props.projectId ? `${props.source === 'curseforge' ? 'CurseForge' : 'Modrinth'} · ${copy.value.title}` : copy.value.title);
const entry = computed(() => key.value ? policy.value.projects[key.value] : undefined);
const ignored = computed(() => entry.value?.ignored_versions ?? []);
const choices = computed(() => [{ value: 'stable', label: copy.value.stable }, { value: 'preview', label: copy.value.preview }]);
let generation = 0;
async function load() {
  const current = ++generation; loaded.value = false; error.value = '';
  try { const result = await invoke<ModUpdatePolicy>('get_game_mod_update_policy', { id: props.gameId }); if (!result || result.schema_version !== 1 || !result.projects) throw new Error('Invalid policy'); if (current === generation) { policy.value = result; loaded.value = true; } }
  catch { if (current === generation) error.value = copy.value.loadFailed; }
}
async function save(next: ModUpdatePolicy) {
  if (!loaded.value || props.disabled || busy.value) return;
  const current = generation; busy.value = true; emit('busy', true); error.value = '';
  try { await invoke('set_game_mod_update_policy', { id: props.gameId, policy: next }); if (current === generation) { policy.value = next; emit('changed'); } }
  catch { if (current === generation) error.value = copy.value.failed; }
  finally { busy.value = false; emit('busy', false); }
}
function channel(value: string, project = false) {
  const next = cloneModUpdatePolicy(policy.value);
  if (project && key.value) {
    const changed = next.projects[key.value] ?? { ignored_versions: [] };
    if (value === 'inherit') delete changed.channel;
    else changed.channel = value as ModUpdateChannel;
    if (changed.channel || changed.ignored_versions.length) next.projects[key.value] = changed;
    else delete next.projects[key.value];
  } else next.channel = value as ModUpdateChannel;
  void save(next);
}
function ignore(version: string, value: boolean) { if (key.value) void save(changeIgnoredVersion(policy.value, key.value, version, value)); }
watch(() => props.gameId, () => { policy.value = emptyModUpdatePolicy(); void load(); }, { immediate: true });
</script>
<template>
  <SettingsDisclosure
    :key="projectId ?? 'game'"
    class="mod-version-policy"
    :title="title"
    :default-open="Boolean(projectId)"
  >
    <p
      v-if="error"
      role="alert"
      class="ui-danger"
    >
      {{ error }} <button
        v-if="!loaded"
        type="button"
        class="text-button"
        @click="load"
      >
        {{ copy.retry }}
      </button>
    </p>
    <SettingsRow
      v-if="!hideGameChannel"
      :label="copy.channel"
      :input-id="`${gameId}-update-channel`"
    >
      <SelectField
        :id="`${gameId}-update-channel`"
        :label="copy.channel"
        :model-value="policy.channel"
        :options="choices"
        :disabled="disabled || busy || !loaded"
        @update:model-value="channel($event)"
      />
    </SettingsRow>
    <SettingsRow
      v-if="projectId"
      :label="copy.projectChannel"
      :input-id="`${gameId}-${source ?? 'modrinth'}-project-channel`"
    >
      <SelectField
        :id="`${gameId}-${source ?? 'modrinth'}-project-channel`"
        :label="copy.projectChannel"
        :model-value="entry?.channel ?? 'inherit'"
        :options="[{ value: 'inherit', label: copy.inherit }, ...choices]"
        :disabled="disabled || busy || !loaded"
        @update:model-value="channel($event, true)"
      />
    </SettingsRow>
    <div
      v-if="projectId"
      class="version-policy-ignored"
    >
      <button
        v-if="selectedVersion"
        type="button"
        class="text-button"
        :disabled="disabled || busy || !loaded"
        @click="ignore(selectedVersion.version_id, !ignored.includes(selectedVersion.version_id))"
      >
        {{ ignored.includes(selectedVersion.version_id) ? copy.resume : copy.ignore }} · {{ selectedVersion.version_number }}
      </button>
      <ul
        v-if="ignored.length"
        :aria-label="copy.ignored"
      >
        <li
          v-for="version in ignored"
          :key="version"
        >
          <span>{{ version }}</span><button
            type="button"
            class="text-button"
            :disabled="disabled || busy"
            @click="ignore(version, false)"
          >
            {{ copy.resume }}
          </button>
        </li>
      </ul>
      <p
        v-if="ignored.length"
        class="ui-caption"
      >
        {{ copy.dependencies }}
      </p>
    </div>
  </SettingsDisclosure>
</template>
<style scoped>
.mod-version-policy{max-width:680px;margin-block:var(--ui-space-4)}
.version-policy-ignored ul{list-style:none;padding:0;margin:var(--ui-space-2) 0;max-height:160px;overflow:auto}
.version-policy-ignored li{display:flex;align-items:center;gap:var(--ui-space-3);justify-content:space-between;font-size:var(--ui-text-caption);overflow-wrap:anywhere}
.version-policy-ignored .text-button{padding-inline:0}
.version-policy-ignored p{margin:var(--ui-space-2) 0}
</style>
