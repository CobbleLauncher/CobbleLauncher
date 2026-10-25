<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import { avatars } from './appearance';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { PlayerProfile } from './games';
import type { SkinViewer } from 'skinview3d';
import LoadingIndicator from './ui/LoadingIndicator.vue';
type Skin = { image: string; avatar: string; model: 'classic' | 'slim' };
type Appearance = { skin: Skin | null; localOnly: boolean };
const props = defineProps<{ profile: PlayerProfile }>();
const emit = defineEmits<{ avatar: [image: string] }>();
const appearance = ref<Appearance | null>(null);
const draft = ref<Skin | null>(null);
const skin = computed(() => draft.value ?? appearance.value?.skin);
const model = ref<'classic' | 'slim'>('classic');
const busy = ref(false); const error = ref('');
const canvas = ref<HTMLCanvasElement | null>(null);
let viewer: SkinViewer | undefined; let disposed = false;
async function renderSkin() {
  if (!canvas.value || !skin.value) return;
  try {
    if (!viewer) {
      const { SkinViewer } = await import('skinview3d');
      if (disposed || !canvas.value) return;
      viewer = new SkinViewer({ canvas: canvas.value, width: 220, height: 260 });
      viewer.autoRotate = false;
      viewer.controls.enableZoom = false;
    }
    await viewer.loadSkin(skin.value.image, { model: model.value === 'slim' ? 'slim' : 'default' });
  } catch { error.value = t('skin.previewError'); }
}
async function load(refresh = false) {
  busy.value = true; error.value = '';
  try {
    const value = await invoke<Appearance>('get_profile_appearance', { id: props.profile.id, refresh });
    if (disposed) return;
    appearance.value = value; if (value.skin) { model.value = value.skin.model; avatars.set(props.profile.id,value.skin.avatar); emit('avatar', value.skin.avatar); }
  } catch { if (!disposed) error.value = t('skin.loadError'); }
  finally { busy.value = false; }
}
async function choose() {
  busy.value = true; error.value = '';
  try { const value = await invoke<Skin | null>('pick_profile_skin', { id: props.profile.id }); if (value && !disposed) { draft.value = value; model.value = value.model; } }
  catch (cause) { error.value = String(cause); }
  finally { busy.value = false; }
}
async function apply() {
  busy.value = true; error.value = '';
  try { appearance.value = await invoke<Appearance>('apply_profile_skin', { id: props.profile.id, model: model.value }); draft.value = null; if (appearance.value.skin) { avatars.set(props.profile.id, appearance.value.skin.avatar); emit('avatar', appearance.value.skin.avatar); } }
  catch (cause) { error.value = String(cause); }
  finally { busy.value = false; }
}
watch([skin, model], () => { void renderSkin(); }, { flush: 'post' });
onMounted(() => { void load(); });
onUnmounted(() => { disposed = true; viewer?.dispose(); });
</script>
<template>
  <div
    class="profile-skin-panel"
    :aria-busy="busy"
  >
    <canvas
      v-show="skin"
      ref="canvas"
      :aria-label="t('skin.preview')"
    />
    <div class="skin-controls">
      <LoadingIndicator
        v-if="busy"
        :label="t('skin.working')"
      />
      <p
        v-else-if="!skin && !error"
        class="setting-hint"
      >
        {{ t('skin.empty') }}
      </p>
      <p
        v-if="profile.kind === 'offline'"
        class="setting-hint"
      >
        {{ t('skin.localOnly') }}
      </p>
      <button
        :class="skin ? 'secondary-button' : 'primary-button'"
        :disabled="busy"
        @click="choose"
      >
        {{ t('skin.choose') }}
      </button>
      <div
        v-if="draft"
        class="form-field"
      >
        <label :for="`skin-model-${profile.id}`">{{ t('skin.model') }}</label>
        <select
          :id="`skin-model-${profile.id}`"
          v-model="model"
          :disabled="busy"
        >
          <option value="classic">
            {{ t('skin.classic') }}
          </option><option value="slim">
            {{ t('skin.slim') }}
          </option>
        </select>
        <button
          class="primary-button"
          :disabled="busy"
          @click="apply"
        >
          {{ profile.kind === 'microsoft' ? t('skin.apply') : t('skin.save') }}
        </button>
        <button
          class="text-button"
          :disabled="busy"
          @click="draft = null"
        >
          {{ t('skin.cancel') }}
        </button>
      </div>
      <button
        v-if="profile.kind === 'microsoft'"
        class="text-button"
        :disabled="busy"
        @click="load(true)"
      >
        {{ t('skin.refresh') }}
      </button>
      <p
        v-if="error"
        role="alert"
        class="settings-error"
      >
        {{ error }}
      </p>
    </div>
  </div>
</template>
<style scoped>
.profile-skin-panel{display:flex;flex-wrap:wrap;align-items:center;gap:16px;padding:16px 0;font-size:var(--ui-text-body,14px);line-height:calc(var(--font-size-base,14px)*22/14)}.skin-controls{display:flex;flex:1;min-width:160px;flex-direction:column;align-items:flex-start;gap:12px}.skin-controls button,.skin-controls label,.skin-controls select{font-size:var(--ui-text-body,14px);line-height:calc(var(--font-size-base,14px)*22/14)}.skin-controls .setting-hint{font-size:var(--ui-text-caption,12px);line-height:calc(var(--font-size-base,14px)*18/14)}canvas{max-width:100%;touch-action:none}
</style>
