<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { t } from './i18n';
const props = defineProps<{ minecraftVersion: string; loaderKind?: string | null; loaderVersion?: string | null }>();
const emit = defineEmits<{ blocked: [value: boolean] }>();
const status = ref('');
const java = ref<number | null>(null);
let request = 0;
watch(() => [props.minecraftVersion, props.loaderKind, props.loaderVersion], async () => {
  const current = ++request;
  status.value = ''; java.value = null;
  if (!['forge', 'neoforge'].includes(props.loaderKind ?? '') || !props.minecraftVersion) { emit('blocked', false); return; }
  emit('blocked', true);
  try {
    if (!isTauri()) return;
    const value = await invoke<{status: string; java_major: number | null}>('query_launch_compatibility', { minecraftVersion: props.minecraftVersion, loaderKind: props.loaderKind, loaderVersion: props.loaderVersion || null });
    if (current !== request) return;
    status.value = value.status; java.value = value.java_major;
    emit('blocked', value.status === 'unsupported');
  } catch { if (current === request) status.value = 'unavailable'; }
}, { immediate: true });
</script>
<template>
  <p
    v-if="status === 'experimental'"
    class="setting-hint"
    role="status"
  >
    {{ t('compatibility.experimental', { java, version: minecraftVersion }) }}
  </p>
  <p
    v-else-if="status === 'unsupported' || status === 'unavailable'"
    class="settings-error"
    role="alert"
  >
    {{ t(status === 'unsupported' ? 'compatibility.unsupported' : 'compatibility.unavailable') }}
  </p>
</template>
