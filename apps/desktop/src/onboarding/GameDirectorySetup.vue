<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale } from '../i18n';
import type { LauncherPreferences } from '../games';
import AppIcon from '../ui/AppIcon.vue';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
import { gameDirectoryCopy } from './game-directory-copy';
type Candidate = { path: string; available_bytes: number | null };
const props = defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean]; ready: [value: boolean] }>();
const copy = computed(() => gameDirectoryCopy[locale.value === 'en-US' ? 'en-US' : 'zh-CN']);
const candidate = ref<Candidate | null>(null);
const savedPath = ref('');
const originallyCustom = ref(false);
const recommended = ref(false);
const pending = ref(true);
const error = ref('');
let revision = 0;
let disposed = false;
const available = computed(() => candidate.value?.available_bytes ?? null);
const pathParts = computed(() => candidate.value?.path.split(/(?<=[\\/])/) ?? []);
const lowSpace = computed(() => available.value !== null && available.value < 10 * 1024 ** 3);
const freeSpace = computed(() => available.value === null ? '' : `${new Intl.NumberFormat(locale.value, {maximumFractionDigits: 1}).format(available.value / 1_000_000_000)} GB`);
watch(pending, value => emit('busy', value), { immediate: true });
watch(candidate, value => emit('ready', !!value?.path), { immediate: true });
async function load() {
  const request = ++revision;
  pending.value = true; error.value = '';
  try {
    const [preferences, current] = await Promise.all([
      invoke<LauncherPreferences>('get_launcher_preferences'),
      invoke<Candidate>('get_default_game_directory_candidate'),
    ]);
    if (disposed || request !== revision) return;
    candidate.value = current; savedPath.value = current.path;
    originallyCustom.value = preferences.custom_game_directory; recommended.value = !preferences.custom_game_directory;
  } catch { if (!disposed && request === revision) error.value = copy.value.loadFailed; }
  finally { if (!disposed && request === revision) pending.value = false; }
}
async function choose(standard = false) {
  if (pending.value || props.disabled) return;
  const request = ++revision;
  pending.value = true; error.value = '';
  try {
    const value = await invoke<Candidate | null>(standard ? 'get_standard_game_directory' : 'pick_default_game_directory_candidate');
    if (disposed || request !== revision || !value) return;
    candidate.value = value;
    recommended.value = standard || (!originallyCustom.value && value.path === savedPath.value);
  } catch { if (!disposed && request === revision) error.value = copy.value.pickFailed; }
  finally { if (!disposed && request === revision) pending.value = false; }
}
async function save() {
  if (pending.value || props.disabled || !candidate.value?.path) return false;
  if (candidate.value.path === savedPath.value && !(recommended.value && originallyCustom.value)) return true;
  const request = ++revision;
  pending.value = true; error.value = '';
  try {
    await invoke(recommended.value ? 'reset_default_game_directory' : 'set_default_game_directory', recommended.value ? undefined : { path: candidate.value.path });
    if (disposed || request !== revision) return false;
    savedPath.value = candidate.value.path; originallyCustom.value = !recommended.value;
    return true;
  } catch { if (!disposed && request === revision) error.value = copy.value.saveFailed; return false; }
  finally { if (!disposed && request === revision) pending.value = false; }
}
defineExpose({ save });
onMounted(load);
onUnmounted(() => { disposed = true; ++revision; emit('busy', false); emit('ready', false); });
</script>
<template>
  <section
    class="setup-game-directory"
    :aria-label="copy.title"
    :aria-busy="pending"
  >
    <div class="setup-directory-heading">
      <AppIcon name="folder" /><span>{{ copy.title }}</span><span
        v-if="recommended && candidate"
        class="setup-directory-recommended"
      >{{ copy.recommended }}</span>
    </div>
    <LoadingIndicator
      v-if="pending && !candidate"
      :label="copy.loading"
    />
    <template v-if="candidate">
      <p
        class="setup-directory-path"
        :title="candidate.path"
      >
        <template
          v-for="(part,index) in pathParts"
          :key="index"
        >
          {{ part }}<wbr>
        </template>
      </p>
      <p
        class="setup-directory-space"
        :class="{'is-low':lowSpace}"
        role="status"
      >
        <AppIcon :name="lowSpace ? 'alert-circle' : 'hard-drive'" />{{ available === null ? copy.unknown : `${copy.available} ${freeSpace}` }}
      </p>
      <p
        v-if="lowSpace"
        class="setup-directory-warning"
      >
        {{ copy.low }}
      </p>
      <div class="setup-directory-actions">
        <button
          type="button"
          class="secondary-button"
          :disabled="pending || disabled"
          @click="choose()"
        >
          <AppIcon name="folder" />{{ copy.browse }}
        </button>
        <button
          v-if="!recommended"
          type="button"
          class="text-button"
          :disabled="pending || disabled"
          @click="choose(true)"
        >
          {{ copy.standard }}
        </button>
      </div>
    </template>
    <p
      v-if="error"
      role="alert"
      class="settings-error setup-directory-error"
    >
      {{ error }}<button
        v-if="!candidate"
        type="button"
        class="text-button"
        :disabled="pending || disabled"
        @click="load"
      >
        {{ copy.retry }}
      </button>
    </p>
  </section>
</template>
<style scoped>
.setup-game-directory{margin-top:var(--ui-space-5);padding-top:var(--ui-space-4);border-top:1px solid var(--ui-border);min-width:0;text-align:left}.setup-directory-heading{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-2);font-size:var(--ui-text-label);line-height:var(--ui-leading-label);font-weight:600}.setup-directory-heading>.app-icon{color:var(--ui-text-muted)}.setup-directory-recommended{margin-left:auto;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-weight:400}.setup-directory-path{margin:var(--ui-space-3) 0 var(--ui-space-2);overflow-wrap:anywhere;font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.setup-directory-space{display:flex;align-items:center;gap:var(--ui-space-2);color:var(--ui-text-muted);margin:0;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-variant-numeric:tabular-nums}.setup-directory-space.is-low,.setup-directory-warning{color:var(--ui-warning)}.setup-directory-warning{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);margin:var(--ui-space-2) 0 0}.setup-directory-actions{display:flex;flex-wrap:wrap;gap:var(--ui-space-2);align-items:center;margin-top:var(--ui-space-3)}.setup-directory-actions>button{font-size:var(--ui-text-label)}.setup-directory-error{margin:var(--ui-space-3) 0 0;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
</style>
