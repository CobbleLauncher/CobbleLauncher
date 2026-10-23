<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import ExtensionSettingsPanel from './ExtensionSettingsPanel.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { pluginLocale, type ExtensionContribution } from './extensions/contributions';
const props = defineProps<{ instanceId: string }>();
const plugins = ref<ExtensionContribution[]>([]);
const error = ref('');
const visited = ref<string[]>([]);
const panels = new Map<string, InstanceType<typeof ExtensionSettingsPanel>>();
const available = computed(() => plugins.value.filter(plugin => plugin.settings.some(category => category.fields?.some(field => field.scope === 'instance'))));
let version = 0;
let disposed = false;
function panelRef(id: string, value: unknown) { if (value) panels.set(id, value as InstanceType<typeof ExtensionSettingsPanel>); else panels.delete(id); }
async function load() {
  const current = ++version; error.value = '';
  try {
    const value = await invoke<ExtensionContribution[]>('list_extension_contributions', { locale: pluginLocale(), context: { instanceId: props.instanceId } });
    if (!disposed && current === version) plugins.value = value;
  } catch { if (!disposed && current === version) error.value = t('extensionCommands.loadError'); }
}
function visit(id: string, open: boolean) { if (open && !visited.value.includes(id)) visited.value.push(id); }
function canLeave() { return [...panels.values()].every(panel => panel.canLeave()); }
async function confirmLeave() { for (const panel of panels.values()) if (!await panel.confirmLeave()) return false; return true; }
watch(() => [locale.value, props.instanceId], () => { void load(); }, { immediate: true });
onUnmounted(() => { disposed = true; ++version; });
defineExpose({ canLeave, confirmLeave });
</script>
<template>
  <section
    v-if="available.length || error"
    class="instance-extension-settings"
  >
    <h2>{{ t('extensionCommands.settings') }}</h2>
    <SettingsDisclosure
      v-for="plugin in available"
      :key="`${instanceId}:${plugin.pluginId}`"
      bordered
      :title="plugin.pluginName"
      @update:open="visit(plugin.pluginId, $event)"
    >
      <ExtensionSettingsPanel
        v-if="visited.includes(plugin.pluginId)"
        :ref="value => panelRef(plugin.pluginId, value)"
        :plugin-id="plugin.pluginId"
        :instance-id="instanceId"
        scope="instance"
      />
    </SettingsDisclosure>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }} <button
        class="text-button"
        @click="load"
      >
        {{ t('extensionCommands.retry') }}
      </button>
    </p>
  </section>
</template>
<style scoped>
.instance-extension-settings{margin-top:24px}.instance-extension-settings>h2{font-size:14px;font-weight:500}.instance-extension-settings :deep(.settings-section){border:0;margin:0;padding:0}
</style>
