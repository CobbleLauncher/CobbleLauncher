<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import PluginAccessLogPanel from './PluginAccessLogPanel.vue';
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t, type TranslationKey } from './i18n';
import PluginPermissions from './PluginPermissions.vue';
import PluginIdentity from './PluginIdentity.vue';
import ExtensionSettingsPanel from './ExtensionSettingsPanel.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { hasGlobalPluginSettings, type PluginDetailsData } from './plugin-details';
import { pluginLocale } from './extensions/contributions';
import type { ExtensionCandidate } from './navigation';
import AppIcon from './ui/AppIcon.vue';
import SwitchField from './ui/SwitchField.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import ConfirmationDialog from './ui/ConfirmationDialog.vue';
import { effectivePermissionNodes } from './plugin-permissions';
import { pluginFallbackIcon } from './plugin-presentation';
const props = defineProps<{ pluginId: string }>();
const accessLogOpen = ref(false);
const emit = defineEmits<{ title: [value: string]; openDetail: [id: string]; browseCatalog: [value: ExtensionCandidate]; removed: []; changed: [] }>();
const details = ref<PluginDetailsData | null>(null);
const settings = ref<InstanceType<typeof ExtensionSettingsPanel> | null>(null);
const busy = ref(false); const loading = ref(false); const error = ref('');
const confirmingUninstall = ref(false);
async function requestUninstall() { const current = generation; const id = props.pluginId; if (!busy.value && await confirmLeave() && current === generation && id === props.pluginId) confirmingUninstall.value = true; }
let generation = 0; let disposed = false;
const ready = computed(() => Boolean(details.value?.plugin.enabled && !details.value.plugin.problem));
const hasSettings = computed(() => Boolean(details.value && hasGlobalPluginSettings(details.value.plugin.manifest)));
const hasPermissions = computed(() => Boolean(details.value && effectivePermissionNodes(details.value.plugin.manifest).length));
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
async function mutate(command: 'set_plugin_enabled' | 'uninstall_plugin', reviewed = false) {
  if (busy.value || !details.value || (!reviewed && !await confirmLeave())) return;
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
watch(()=>props.pluginId,()=>{confirmingUninstall.value=false;});
onUnmounted(() => { disposed = true; ++generation; });
</script>
<template>
  <div
    class="page-content plugin-details"
    :aria-busy="busy || loading"
  >
    <LoadingIndicator
      v-if="loading && !details"
      :label="t('pluginDetails.loading')"
    />
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
            :fallback-icon="pluginFallbackIcon(details.plugin.manifest)"
            fetch-icon
            :show-name="false"
          />
          <p class="plugin-details-meta">
            {{ details.plugin.manifest.publisher }} · {{ details.plugin.manifest.version }}
          </p>
          <p
            v-if="details.signature"
            class="setting-hint plugin-signature-status"
            :title="details.signature.rootId"
          >
            <AppIcon :name="details.signature.status === 'verified' ? 'shield' : 'info'" />
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
          <div class="plugin-enabled-control">
            <SwitchField
              :model-value="details.plugin.enabled"
              :label="t('plugins.enabledState', { name: details.plugin.manifest.name })"
              :disabled="busy || (!!details.plugin.problem && !(details.plugin.enabled && details.plugin.problem === 'PLUGIN_MISSING_INTERFACE'))"
              @update:model-value="mutate('set_plugin_enabled')"
            /><span>{{ t(details.plugin.enabled ? 'plugins.catalogEnabled' : 'plugins.catalogDisabled') }}</span>
          </div>
          <button
            class="text-button ui-danger"
            :disabled="busy"
            @click="requestUninstall"
          >
            <AppIcon name="trash" />{{ t('plugins.uninstall') }}
          </button>
        </div>
      </section>
      <section
        v-if="hasPermissions"
        class="plugin-details-permissions"
      >
        <h2>{{ t('pluginPermissions.title') }}</h2>
        <PluginPermissions :manifest="details.plugin.manifest" />
      </section>
      <SettingsDisclosure
        icon="info"
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
        icon="puzzle"
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
    <ConfirmationDialog
      v-model:open="confirmingUninstall"
      :title="t('plugins.uninstallTitle', { name: details?.plugin.manifest.name ?? pluginId })"
      :description="t('plugins.uninstallWarning')"
      :confirm-label="t('plugins.uninstall')"
      danger
      :disabled="busy || loading"
      @confirm="mutate('uninstall_plugin', true)"
    />
  </div>
</template>
<style scoped>
.plugin-details{min-width:0;max-width:1000px}.plugin-details-overview{display:flex;align-items:flex-start;justify-content:space-between;flex-wrap:wrap;gap:var(--ui-space-4);min-width:0;padding:0 0 var(--ui-space-4);border-bottom:1px solid var(--ui-border)}.plugin-details-summary{min-width:0;flex:1 1 300px}.plugin-details-summary :deep(.plugin-identity){gap:var(--ui-space-3)}.plugin-details-summary :deep(.plugin-identity-copy>p){font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.plugin-details-summary :deep(.plugin-identity-tags li){font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.plugin-details-meta{color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);margin:var(--ui-space-2) 0 0 56px;overflow-wrap:anywhere}.plugin-details-summary>.plugin-signature-status{display:flex;align-items:center;gap:var(--ui-space-2);margin:var(--ui-space-2) 0 0 56px;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.plugin-details-actions{display:flex;align-items:center;gap:var(--ui-space-4);flex-wrap:wrap}.plugin-enabled-control{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}.plugin-details-actions>button{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);max-width:100%;font-size:var(--ui-text-label)}.plugin-details-permissions{display:grid;grid-template-columns:180px minmax(0,1fr);gap:var(--ui-space-5);padding:var(--ui-space-5) 0}.plugin-details-permissions>h2{margin:var(--ui-space-1) 0 0;font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:600}.plugin-details-permissions :deep(.plugin-permissions){margin:0}
.plugin-details>.settings-disclosure{margin-block:0}.plugin-details>.settings-disclosure+.settings-disclosure,.plugin-details-overview+.settings-disclosure{border-top:0}
.plugin-combinations{margin:var(--ui-space-5) 0}.plugin-combinations h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:600;margin:0 0 var(--ui-space-3)}.plugin-dependency{border-bottom:1px solid var(--ui-border);padding:var(--ui-space-3) 0;margin:var(--ui-space-3) 0;min-width:0}.plugin-dependency-title{display:flex;justify-content:space-between;gap:var(--ui-space-3);flex-wrap:wrap;font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.plugin-dependency-title>span{color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.plugin-dependency ul,.plugin-companion-list{list-style:none;padding:0;margin:var(--ui-space-2) 0 0}.plugin-dependency li,.plugin-companion-list li{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);flex-wrap:wrap;padding:var(--ui-space-3) 0;min-width:0}.plugin-dependency li>span,.plugin-companion-list li>span{min-width:0;overflow-wrap:anywhere;font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.plugin-dependency small,.plugin-companion-list small{display:block;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);margin-top:var(--ui-space-1)}.plugin-companion-list li{border-bottom:1px solid var(--ui-border)}.plugin-manifest{box-sizing:border-box;width:100%;max-height:min(60vh,560px);overflow:auto;overscroll-behavior:contain;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font-family:var(--ui-font-mono);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);margin:0;padding:var(--ui-space-3) 0}
@media(max-width:680px){.plugin-details-permissions{grid-template-columns:minmax(0,1fr);gap:var(--ui-space-3);padding:var(--ui-space-4) 0}.plugin-details-permissions>h2{margin:0}}
</style>
