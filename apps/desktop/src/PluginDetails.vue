<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import PluginAccessLogPanel from './PluginAccessLogPanel.vue';
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t, type TranslationKey } from './i18n';
import PluginPermissions from './PluginPermissions.vue';
import PluginPermissionBadges from './PluginPermissionBadges.vue';
import PluginIdentity from './PluginIdentity.vue';
import ExtensionSettingsPanel from './ExtensionSettingsPanel.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { hasGlobalPluginSettings, type PluginDetailsData } from './plugin-details';
import { pluginLocale } from './extensions/contributions';
import type { ExtensionCandidate } from './navigation';
import AppIcon from './ui/AppIcon.vue';
const props = defineProps<{ pluginId: string }>();
const accessLogOpen = ref(false);
const emit = defineEmits<{ title: [value: string]; openDetail: [id: string]; browseCatalog: [value: ExtensionCandidate]; removed: []; changed: [] }>();
const details = ref<PluginDetailsData | null>(null);
const settings = ref<InstanceType<typeof ExtensionSettingsPanel> | null>(null);
const busy = ref(false); const loading = ref(false); const error = ref('');
let generation = 0; let disposed = false;
const ready = computed(() => Boolean(details.value?.plugin.enabled && !details.value.plugin.problem));
const hasSettings = computed(() => Boolean(details.value && hasGlobalPluginSettings(details.value.plugin.manifest)));
const manifestText = computed(() => details.value ? JSON.stringify(details.value.plugin.manifest, null, 2) : '');
function problemLabel(problem: string) { return t(problem === 'PLUGIN_MISSING_INTERFACE' ? 'plugins.missingInterface' : problem === 'PLUGIN_UNSUPPORTED_API' ? 'plugins.catalogUnsupported' : 'plugins.damaged'); }
function interfaceLabel(id: string) {
  const labels: Record<string, TranslationKey> = { 'launcher:viewer': 'pluginDetails.viewer', 'launcher:report': 'pluginDetails.report', 'launcher:blueprint-provider': 'pluginDetails.provider' };
  return labels[id] ? t(labels[id]) : id;
}
async function load() {
  const current = ++generation; const id = props.pluginId;
  loading.value = true; error.value = '';
  if (details.value?.plugin.manifest.id !== id) details.value = null;
  try {
    const value = await invoke<PluginDetailsData>('get_plugin_details', { id, locale: pluginLocale() });
    if (disposed || generation !== current) return;
    if (value.plugin.manifest.id !== id) throw new Error('Invalid plugin identity');
    details.value = value; emit('title', value.plugin.manifest.name);
  } catch { if (!disposed && generation === current) error.value = t('pluginDetails.loadFailed'); }
  finally { if (!disposed && generation === current) loading.value = false; }
}
async function mutate(command: 'set_plugin_enabled' | 'uninstall_plugin') {
  if (busy.value || !details.value || !await confirmLeave()) return;
  const id = props.pluginId; const current = generation; busy.value = true; error.value = '';
  try {
    await invoke(command, { id, ...(command === 'set_plugin_enabled' ? { enabled: !details.value.plugin.enabled } : {}) });
    if (disposed || current !== generation) return;
    busy.value = false; emit('changed');
    if (command === 'uninstall_plugin') emit('removed'); else await load();
  } catch { if (!disposed && current === generation) error.value = t('pluginDetails.actionFailed'); }
  finally { if (!disposed && props.pluginId === id) busy.value = false; }
}
function canLeave() { return !busy.value && (settings.value?.canLeave() ?? true); }
async function confirmLeave() { return !busy.value && (await settings.value?.confirmLeave() ?? true); }
defineExpose({ canLeave, confirmLeave });
watch(() => [props.pluginId, locale.value], (current, previous) => { if (previous?.[0] === current[0] && busy.value) return; void load(); }, { immediate: true });
onUnmounted(() => { disposed = true; ++generation; });
</script>
<template>
  <div
    class="page-content plugin-details"
    :aria-busy="busy || loading"
  >
    <p
      v-if="loading && !details"
      role="status"
    >
      {{ t('pluginDetails.loading') }}
    </p>
    <template v-if="details">
      <p
        v-if="details.plugin.problem"
        class="settings-error"
        role="status"
      >
        {{ problemLabel(details.plugin.problem) }}
      </p>
      <section class="plugin-details-overview">
        <div class="plugin-details-summary">
          <PluginIdentity
            :manifest="details.plugin.manifest"
            fetch-icon
            :show-name="false"
          />
          <p class="plugin-details-meta">
            {{ details.plugin.manifest.publisher }} · {{ details.plugin.manifest.version }}
          </p>
          <PluginPermissionBadges :manifest="details.plugin.manifest" />
          <p
            v-if="details.signature"
            class="setting-hint plugin-signature-status"
            :title="details.signature.rootId"
          >
            {{ details.signature.status === 'verified' ? t('pluginPermissions.signedBy', { publisher: details.signature.publisher! }) : t('pluginPermissions.unsigned') }}
          </p>
          <p
            v-else
            class="settings-error"
          >
            {{ t('pluginPermissions.signatureFailed') }}
          </p>
        </div>
        <div class="plugin-details-actions">
          <button
            class="text-button"
            :disabled="busy || (!!details.plugin.problem && !(details.plugin.enabled && details.plugin.problem === 'PLUGIN_MISSING_INTERFACE'))"
            @click="mutate('set_plugin_enabled')"
          >
            <AppIcon :name="details.plugin.enabled ? 'x' : 'check'" />{{ t(details.plugin.enabled ? 'plugins.disable' : 'plugins.enable') }}
          </button>
          <button
            class="text-button"
            :disabled="busy"
            @click="mutate('uninstall_plugin')"
          >
            <AppIcon name="trash" />{{ t('plugins.uninstall') }}
          </button>
        </div>
      </section>
      <section class="plugin-details-permissions">
        <h2>{{ t('pluginPermissions.title') }}</h2><PluginPermissions :manifest="details.plugin.manifest" />
      </section>
      <SettingsDisclosure
        :title="t('pluginAccessLog.title')"
        @update:open="accessLogOpen = $event"
      >
        <PluginAccessLogPanel
          v-if="accessLogOpen"
          :plugin-id="pluginId"
          embedded
        />
      </SettingsDisclosure>
      <ExtensionSettingsPanel
        v-if="hasSettings && ready"
        :key="pluginId"
        ref="settings"
        :plugin-id="pluginId"
        scope="global"
      />
      <section
        v-if="details.dependencies.length"
        class="plugin-combinations"
      >
        <h2>{{ t('pluginDetails.combinations') }}</h2>
        <div
          v-for="dependency in details.dependencies"
          :key="dependency.requirement.id"
          class="plugin-dependency"
        >
          <div class="plugin-dependency-title">
            <strong>{{ interfaceLabel(dependency.requirement.id) }}</strong><span>{{ t(dependency.optional ? 'pluginDetails.optional' : 'pluginDetails.required') }}</span>
          </div>
          <p
            v-if="dependency.state !== 'available'"
            class="setting-hint"
          >
            {{ t(`pluginDetails.${dependency.state}`) }}
          </p>
          <ul>
            <li
              v-for="item in dependency.implementations"
              :key="item.plugin_id"
            >
              <span>{{ item.name }} <small>{{ item.version }} · {{ t(item.available ? 'pluginDetails.available' : 'pluginDetails.inactive') }}</small></span><button
                class="text-button"
                :disabled="busy"
                @click="emit('openDetail', item.plugin_id)"
              >
                {{ t('pluginDetails.details') }}
              </button>
            </li>
            <li
              v-for="item in dependency.candidates"
              :key="`${item.source_id}:${item.plugin_id}`"
            >
              <span>{{ item.name }} <small>{{ item.version }} · {{ item.source_name }}</small></span><button
                class="text-button"
                :disabled="busy"
                @click="emit('browseCatalog', { sourceId: item.source_id, pluginId: item.plugin_id })"
              >
                {{ t('pluginDetails.reviewInstall') }}
              </button>
            </li>
          </ul>
        </div>
      </section>
      <section
        v-if="details.companions?.length"
        class="plugin-combinations"
      >
        <h2 v-if="!details.dependencies.length">
          {{ t('pluginDetails.combinations') }}
        </h2>
        <ul class="plugin-companion-list">
          <li
            v-for="item in details.companions"
            :key="`${item.source_id ?? 'installed'}:${item.plugin_id}`"
          >
            <span><strong>{{ item.name }}</strong><small>{{ item.version }} · {{ item.installed ? t(item.available ? 'pluginDetails.available' : 'pluginDetails.inactive') : item.source_name }}</small></span><button
              v-if="item.installed"
              class="text-button"
              :disabled="busy"
              @click="emit('openDetail', item.plugin_id)"
            >
              {{ t('pluginDetails.details') }}
            </button><button
              v-else-if="item.source_id"
              class="text-button"
              :disabled="busy"
              @click="emit('browseCatalog', { sourceId: item.source_id, pluginId: item.plugin_id })"
            >
              {{ t('pluginDetails.reviewInstall') }}
            </button>
          </li>
        </ul>
      </section>
      <SettingsDisclosure
        bordered
        :title="t('pluginDetails.manifest')"
      >
        <pre class="plugin-manifest">{{ manifestText }}</pre>
      </SettingsDisclosure>
    </template>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <button
      v-if="error"
      class="text-button"
      :disabled="busy || loading"
      @click="load"
    >
      {{ t('plugins.retry') }}
    </button>
  </div>
</template>
<style scoped>
.plugin-details{min-width:0}.plugin-details-overview{display:grid;gap:12px;min-width:0;padding:16px;border:1px solid var(--line);border-radius:12px;background:var(--panel)}.plugin-details-summary{min-width:0}.plugin-details-meta{color:var(--muted);font-size:12px;margin:6px 0 0 56px;overflow-wrap:anywhere}.plugin-details-actions{display:flex;align-items:center;gap:8px;flex-wrap:wrap}.plugin-details-actions>button{display:inline-flex;align-items:center;gap:7px;min-width:0;max-width:100%;overflow-wrap:anywhere}.plugin-details-actions>button :deep(svg){width:16px;height:16px;flex:none}.plugin-combinations{margin:24px 0}.plugin-combinations h2{font-size:14px;font-weight:600;margin:0 0 12px}.plugin-dependency{border:1px solid var(--line);border-radius:12px;background:var(--panel);padding:16px;margin:12px 0;min-width:0}.plugin-dependency-title{display:flex;justify-content:space-between;gap:12px;flex-wrap:wrap;font-size:13px}.plugin-dependency-title>span{color:var(--muted);font-size:11px}.plugin-dependency ul{list-style:none;padding:0;margin:10px 0 0}.plugin-dependency li{display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap;padding:8px 0}.plugin-dependency li>span{min-width:0;overflow-wrap:anywhere}.plugin-dependency small{display:block;color:var(--muted);font-size:11px;margin-top:3px}.plugin-companion-list{list-style:none;margin:0;padding:0}.plugin-companion-list li{display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;padding:16px;border:1px solid var(--line);border-radius:12px;margin:10px 0;background:var(--panel)}.plugin-companion-list li>span{min-width:0;overflow-wrap:anywhere;font-size:13px}.plugin-companion-list small{display:block;font-size:11px;color:var(--muted);margin-top:4px}.plugin-manifest{box-sizing:border-box;width:100%;max-height:min(60vh,560px);overflow:auto;overscroll-behavior:contain;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font-family:inherit;font-size:12px;line-height:1.6;margin:0;padding:12px 0}
</style>

<style scoped>.plugin-details-permissions{margin:22px 0}.plugin-details-permissions h2{font-size:14px;margin:0}</style>
