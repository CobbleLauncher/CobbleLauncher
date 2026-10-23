<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import type { CatalogManifest } from './plugin-catalogs';
import LoadingIndicator from './ui/LoadingIndicator.vue';

type Signature = { status: 'unsigned' | 'verified'; rootId?: string; publisher?: string };
const props = defineProps<{ manifest: CatalogManifest }>();
const emit = defineEmits<{ ready: [value: boolean] }>();
const result = ref<Signature>();
const failed = ref(false);
const loading = ref(false);
let revision = 0;
async function load() {
  const current = ++revision;
  emit('ready', false);
  result.value = undefined;
  failed.value = false;
  loading.value = true;
  try {
    const value = await invoke<Signature>('get_pending_plugin_signature');
    if (current !== revision) return;
    if (!value || !['unsigned', 'verified'].includes(value.status)
      || (value.status === 'verified' && (!value.rootId || !value.publisher))) throw new Error('invalid signature status');
    result.value = value;
    emit('ready', true);
  } catch {
    if (current === revision) failed.value = true;
  } finally {
    if (current === revision) loading.value = false;
  }
}
watch(() => props.manifest, () => { void load(); }, { immediate: true });
onUnmounted(() => { ++revision; });
</script>
<template>
  <LoadingIndicator
    v-if="loading"
    :label="t('pluginPermissions.checkingSignature')"
  />
  <p
    v-else-if="failed"
    class="settings-error"
    role="alert"
  >
    {{ t('pluginPermissions.signatureFailed') }}
    <button
      class="text-button"
      @click="load"
    >
      {{ t('navigation.retry') }}
    </button>
  </p>
  <p
    v-else-if="result"
    class="setting-hint plugin-signature-status"
    :title="result.rootId"
  >
    {{ result.status === 'verified' ? t('pluginPermissions.signedBy', { publisher: result.publisher! }) : t('pluginPermissions.unsigned') }}
  </p>
</template>
