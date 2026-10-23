<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DropdownMenuItem } from 'reka-ui';
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import { pluginLocale, type ExtensionContext, type ExtensionContribution, type ResolvedCommand } from './extensions/contributions';
const props = defineProps<{ position: 'tools' | 'instance' | 'blueprint'; context?: ExtensionContext; disabled?: boolean; revision?: number }>();
const emit = defineEmits<{ selected: [command: ResolvedCommand]; error: [message: string] }>();
const contributions = ref<ExtensionContribution[]>([]);
const resolving = ref(false);
const commands = computed(() => contributions.value.flatMap(plugin => plugin.commands.filter(command => command.menus.some(menu => menu.position === props.position)).map(command => ({ ...command, pluginId: plugin.pluginId }))));
let loadVersion = 0;
let contextVersion = 0;
let disposed = false;
const context = () => ({ position: props.position, locale: pluginLocale(), ...(props.context ? { context: { ...props.context } } : {}) });
async function load() {
  const current = ++loadVersion;
  contributions.value = [];
  try {
    const value = await invoke<ExtensionContribution[]>('list_extension_contributions', context());
    if (!disposed && current === loadVersion) contributions.value = value;
  } catch { if (!disposed && current === loadVersion) emit('error', t('extensionCommands.loadError')); }
}
async function select(pluginId: string, commandId: string) {
  if (props.disabled || resolving.value) return;
  const current = contextVersion;
  resolving.value = true;
  try {
    const command = await invoke<ResolvedCommand>('resolve_extension_command', { pluginId, commandId, ...context() });
    if (!disposed && current === contextVersion) emit('selected', command);
  } catch { if (!disposed && current === contextVersion) emit('error', t('extensionCommands.commandError')); }
  finally { resolving.value = false; }
}
watch(() => [locale.value, props.position, props.context?.instanceId, props.context?.blueprintId, props.revision], () => { ++contextVersion; resolving.value = false; void load(); }, { immediate: true });
onUnmounted(() => { disposed = true; ++loadVersion; });
</script>
<template>
  <DropdownMenuItem
    v-for="command in commands"
    :key="`${command.pluginId}:${command.id}`"
    class="profile-menu-item"
    :disabled="disabled || resolving"
    @select.prevent="select(command.pluginId, command.id)"
  >
    <AppIcon name="puzzle" />{{ command.label }}
  </DropdownMenuItem>
</template>
