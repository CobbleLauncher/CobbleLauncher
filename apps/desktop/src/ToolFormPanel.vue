<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, defineAsyncComponent, onUnmounted, ref, useId, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { t } from './i18n';
import { pluginLocale } from './extensions/contributions';
import AppIcon from './ui/AppIcon.vue';
import { toolFormDefaults, toolFormUiSchema, type ToolFormDefinition, type ToolFormOutput } from './tool-forms';
const SchemaSettingsForm = defineAsyncComponent(() => import('./extensions/forms/SchemaSettingsForm.vue'));
const props = defineProps<{ pluginId: string; form: ToolFormDefinition; contextVersion?: string; disabled?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const input = ref<Record<string, unknown>>({});
const output = ref<ToolFormOutput | null>(null);
const busy = ref(false);
const cancelling = ref(false);
const error = ref('');
const copied = ref<number | null>(null);
const outputId = useId();
let generation = 0;
let disposed = false;
let runningPlugin: string | null = null;
const presentation = computed(() => {
  try { return { ui: toolFormUiSchema(props.form.ui_schema), valid: true }; }
  catch { return { ui: undefined, valid: false }; }
});
watch(() => [props.pluginId, JSON.stringify([props.form.schema,props.form.ui_schema])], () => {
  if (runningPlugin) void invoke('cancel_plugin_tool_form',{pluginId:runningPlugin}).catch(() => {});
  ++generation; input.value = toolFormDefaults(props.form,props.contextVersion); output.value = null; error.value = ''; copied.value = null;
}, { immediate:true });
function failure(cause: unknown) {
  if (typeof cause === 'object' && cause !== null && 'code' in cause && cause.code === 'PLUGIN_TOOL_INPUT' && 'message' in cause
    && typeof cause.message === 'string' && cause.message.trim() && new TextEncoder().encode(cause.message).length <= 512
    && !Array.from(cause.message).some(character => { const code=character.charCodeAt(0); return code < 32 || (code >= 127 && code <= 159); })) return cause.message;
  const code = String(cause);
  return code.includes('PLUGIN_INVALID_TOOL_FORM') ? t('toolForms.invalidInput')
    : /PLUGIN_(DISABLED|NOT_FOUND|CHANGED)/.test(code) ? t('toolForms.unavailable') : t('toolForms.failed');
}
function change(values: Record<string, unknown>) {
  const keys = Object.keys(values);
  if (keys.length === Object.keys(input.value).length && keys.every(key => Object.is(values[key],input.value[key]))) return;
  input.value = values; output.value = null; copied.value = null; error.value = '';
}
async function generate() {
  if (busy.value || props.disabled || !presentation.value.valid) return;
  const current = generation; const pluginId = props.pluginId;
  busy.value = true; runningPlugin = pluginId; cancelling.value = false; error.value = ''; copied.value = null; output.value = null; emit('busy',true);
  try {
    const result = await invoke<ToolFormOutput | null>('run_plugin_tool_form',{ pluginId,input:{...input.value},locale:pluginLocale() });
    if (!disposed && generation === current && !cancelling.value) output.value = result;
  } catch (cause) { if (!disposed && generation === current && !cancelling.value) error.value = failure(cause); }
  finally { busy.value = false; cancelling.value = false; runningPlugin = null; if (!disposed) emit('busy',false); }
}
async function cancel() {
  if (!runningPlugin || cancelling.value) return;
  cancelling.value = true;
  try { await invoke('cancel_plugin_tool_form',{pluginId:runningPlugin}); }
  catch (cause) { if (!disposed) { cancelling.value = false; error.value = failure(cause); } }
}
async function copy(index: number) {
  const report = output.value; const block = report?.blocks[index]; if (!block) return;
  try { await writeText(block.text); if (!disposed && output.value === report) copied.value = index; }
  catch { if (!disposed) error.value = t('toolForms.copyFailed'); }
}
onUnmounted(() => { disposed = true; ++generation; if (busy.value) emit('busy',false); if (runningPlugin) void invoke('cancel_plugin_tool_form',{pluginId:runningPlugin}).catch(() => {}); });
defineExpose({ cancel, confirmLeave: async () => { await cancel(); return true; } });
</script>
<template>
  <section
    class="tool-form-panel"
    :aria-busy="busy"
  >
    <form
      v-if="presentation.valid"
      @submit.prevent="generate"
    >
      <SchemaSettingsForm
        :schema="form.schema"
        :ui-schema="form.ui_schema"
        :translations="form.translations?.[pluginLocale()]"
        :data="input"
        :disabled="busy || disabled"
        @change="change"
      />
      <div class="tool-form-actions">
        <button
          class="primary-button"
          type="submit"
          :disabled="busy || disabled"
        >
          <AppIcon :name="busy ? 'loader-circle' : 'play'" />{{ busy ? t('toolForms.generating') : t('toolForms.generate') }}
        </button>
        <button
          v-if="busy"
          class="secondary-button"
          type="button"
          :disabled="cancelling"
          @click="cancel"
        >
          <AppIcon name="x" />{{ cancelling ? t('toolForms.cancelling') : t('toolForms.cancel') }}
        </button>
      </div>
    </form>
    <p
      v-else
      role="alert"
      class="tool-form-error"
    >
      {{ t('toolForms.invalidForm') }}
    </p>
    <p
      v-if="error"
      role="alert"
      class="tool-form-error"
    >
      {{ error }}
    </p>
    <section
      v-if="output"
      class="tool-form-output"
      :aria-label="output.title"
      aria-live="polite"
    >
      <h3>{{ output.title }}</h3>
      <div
        v-for="(block,index) in output.blocks"
        :key="index"
        class="tool-form-block"
      >
        <div class="tool-form-output-actions">
          <label
            v-if="block.label"
            :for="`${outputId}-${index}`"
          >{{ block.label }}</label>
          <button
            type="button"
            class="secondary-button"
            :aria-label="t('toolForms.copyBlock',{label:block.label ?? output.title})"
            @click="copy(index)"
          >
            <AppIcon :name="copied === index ? 'check' : 'copy'" />{{ copied === index ? t('toolForms.copied') : t('toolForms.copy') }}
          </button>
        </div>
        <textarea
          :id="`${outputId}-${index}`"
          readonly
          spellcheck="false"
          :value="block.text"
          :aria-label="block.label ?? output.title"
          :rows="Math.min(12,Math.max(3,block.text.split('\n').length + 1))"
        />
      </div>
    </section>
  </section>
</template>
<style scoped>
.tool-form-panel,.tool-form-panel form,.tool-form-output,.tool-form-block{display:grid;gap:var(--ui-space-4);min-width:0}.tool-form-panel{font-family:var(--ui-font-sans)}
.tool-form-actions,.tool-form-output-actions{display:flex;align-items:center;gap:var(--ui-space-2);flex-wrap:wrap}
.tool-form-output-actions{justify-content:space-between}.tool-form-output-actions button{margin-left:auto}
.tool-form-output h3{margin:0;font-size:var(--ui-text-section);font-weight:var(--ui-weight-semibold)}
.tool-form-block{gap:var(--ui-space-2)}.tool-form-block textarea{box-sizing:border-box;width:100%;min-width:0;resize:vertical;font:var(--ui-text-label)/1.6 var(--ui-font-mono);white-space:pre;overflow:auto;color:var(--ui-text);background:var(--ui-surface-1);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);padding:var(--ui-space-3)}
.tool-form-block textarea:focus-visible{outline:2px solid var(--ui-focus);outline-offset:2px}
.tool-form-error{color:var(--ui-danger);margin:0;overflow-wrap:anywhere}
</style>
