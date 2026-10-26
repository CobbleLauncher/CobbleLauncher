<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import type { GameVersion } from './games';
import { t } from './i18n';
import SearchField from './ui/SearchField.vue';
import AppIcon from './ui/AppIcon.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
const props = defineProps<{ id: string; modelValue: string; versions: GameVersion[]; disabled?: boolean; loading?: boolean; labelledBy?: string; allowAny?: boolean; preferredVersions?: string[] }>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
type VersionKind = 'release' | 'snapshot' | 'legacy';
const expanded = ref(false);
const kind = ref<VersionKind>('release');
const query = ref('');
const limit = ref(20);
const releases = computed(() => props.versions.filter(version => version.kind === 'release'));
function category(version: GameVersion): VersionKind { return version.kind === 'release' ? 'release' : version.kind === 'old_alpha' || version.kind === 'old_beta' ? 'legacy' : 'snapshot'; }
const matches = computed(() => props.versions.filter(version => category(version) === kind.value && version.id.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())));
const tiles = computed(() => {
  const preferred = (props.preferredVersions ?? []).map(id => props.versions.find(version => version.id === id)).filter((version): version is GameVersion => !!version);
  const quick = [...new Map([...preferred, ...releases.value].map(version => [version.id, version])).values()].slice(0, 5);
  const result = expanded.value ? matches.value.slice(0, limit.value) : quick;
  const selected = props.versions.find(version => version.id === props.modelValue);
  return selected && !result.some(version => version.id === selected.id) ? [...result, selected] : result;
});
function browse(value: VersionKind) { expanded.value = true; kind.value = value; query.value = ''; limit.value = 20; }
function search(value: string) { query.value = value; limit.value = 20; }
function select(value: unknown) { if (typeof value === 'string') emit('update:modelValue', value); }
</script>
<template>
  <div class="minecraft-versions">
    <LoadingIndicator
      v-if="loading"
      :label="t('createGame.loadingVersions')"
    />
    <template v-else>
      <div
        v-if="expanded"
        class="minecraft-version-browser"
      >
        <SearchField
          :id="`${id}-search`"
          :model-value="query"
          :label="t('versions.search')"
          :placeholder="t('versions.search')"
          :clear-label="t('tools.clearSearch')"
          :disabled="disabled"
          @update:model-value="search"
        />
        <div
          class="minecraft-version-kinds"
          role="group"
          :aria-label="t('versions.versionKind')"
        >
          <button
            v-for="value in (['release','snapshot','legacy'] as const)"
            :key="value"
            type="button"
            :aria-pressed="kind === value"
            :disabled="disabled"
            @click="browse(value)"
          >
            {{ t(`versions.${value}`) }}
          </button>
        </div>
      </div>
      <RadioGroupRoot
        :id="id"
        :model-value="modelValue"
        :disabled="disabled"
        :aria-labelledby="labelledBy"
        :aria-label="labelledBy ? undefined : t('createGame.version')"
        class="minecraft-version-grid"
        @update:model-value="select"
      >
        <RadioGroupItem
          v-if="allowAny"
          value=""
          class="minecraft-version-tile"
        >
          <span class="minecraft-version-number">{{ t('market.anyVersion') }}</span>
          <AppIcon
            v-if="!modelValue"
            name="check"
          />
        </RadioGroupItem>
        <RadioGroupItem
          v-for="item in tiles"
          :key="item.id"
          :value="item.id"
          class="minecraft-version-tile"
        >
          <span class="minecraft-version-number">{{ item.id }}</span>
          <AppIcon
            v-if="item.id === modelValue"
            name="check"
          />
          <span
            v-if="item.id === releases[0]?.id"
            class="minecraft-version-note"
          >{{ t('versions.latest') }}</span>
          <span
            v-else-if="item.id === modelValue"
            class="minecraft-version-note"
          >{{ t('versions.selected') }}</span>
          <span
            v-else-if="category(item) !== 'release'"
            class="minecraft-version-note"
          >{{ t(`versions.${category(item)}`) }}</span>
        </RadioGroupItem>
      </RadioGroupRoot>
      <p
        v-if="expanded && !matches.length"
        class="minecraft-version-empty"
        role="status"
      >
        {{ t('versions.empty') }}
      </p>
      <div
        v-if="versions.length"
        class="minecraft-version-actions"
      >
        <button
          v-if="expanded && matches.length > limit"
          type="button"
          class="text-button"
          :disabled="disabled"
          @click="limit += 20"
        >
          {{ t('versions.more') }}
        </button>
        <button
          v-if="!expanded"
          type="button"
          class="text-button"
          :disabled="disabled"
          @click="browse('release')"
        >
          {{ t('versions.browse') }}<AppIcon name="chevron-down" />
        </button>
        <template v-if="!expanded">
          <button
            type="button"
            class="text-button"
            :disabled="disabled"
            @click="browse('snapshot')"
          >
            {{ t('versions.snapshot') }}
          </button>
          <button
            type="button"
            class="text-button"
            :disabled="disabled"
            @click="browse('legacy')"
          >
            {{ t('versions.legacy') }}
          </button>
        </template>
        <button
          v-else
          type="button"
          class="text-button minecraft-version-collapse"
          :disabled="disabled"
          @click="expanded = false"
        >
          {{ t('versions.collapse') }}
        </button>
      </div>
    </template>
  </div>
</template>
<style scoped>
.minecraft-versions{width:100%;min-width:0}.minecraft-version-grid{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:var(--ui-space-2)}.minecraft-version-tile{position:relative;display:grid;grid-template-columns:minmax(0,1fr) 16px;grid-template-rows:auto 18px;align-content:center;gap:var(--ui-space-1);min-height:76px;padding:var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);text-align:left;cursor:pointer;font:inherit}.minecraft-version-tile:hover:not(:disabled){background:var(--ui-surface-hover)}.minecraft-version-tile[data-state=checked]{border-color:var(--ui-focus);background:var(--tint);box-shadow:inset 0 0 0 1px var(--ui-focus)}.minecraft-version-tile:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}.minecraft-version-tile:disabled{opacity:.55;cursor:not-allowed}.minecraft-version-number{grid-column:1/-1;font-size:var(--ui-text-body);line-height:var(--ui-leading-body);font-weight:600;overflow-wrap:anywhere;font-variant-numeric:tabular-nums}.minecraft-version-tile>.app-icon{grid-column:2;grid-row:2;color:var(--ui-focus)}.minecraft-version-note{grid-column:1;grid-row:2;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.minecraft-version-actions{display:flex;flex-wrap:wrap;gap:var(--ui-space-3);align-items:center;margin-top:var(--ui-space-2)}.minecraft-version-actions button{display:inline-flex;align-items:center;gap:var(--ui-space-1);min-height:var(--ui-control-compact);padding:0;font-size:var(--ui-text-caption)}.minecraft-version-collapse{margin-left:auto}.minecraft-version-browser{display:grid;gap:var(--ui-space-3);margin-bottom:var(--ui-space-3)}.minecraft-version-kinds{display:flex;gap:var(--ui-space-2);flex-wrap:wrap}.minecraft-version-kinds button{min-height:var(--ui-control-compact);padding:0 var(--ui-space-3);border:1px solid transparent;border-radius:var(--ui-radius-sm);background:transparent;color:var(--ui-text-muted);font:inherit;font-size:var(--ui-text-label)}.minecraft-version-kinds button[aria-pressed=true]{background:var(--ui-surface-2);border-color:var(--ui-border);color:var(--ui-text);font-weight:600}.minecraft-version-empty{margin:var(--ui-space-3) 0;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}
@media(max-width:480px){.minecraft-version-grid{grid-template-columns:repeat(3,minmax(0,1fr))}}
</style>
