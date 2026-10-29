<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, useId } from 'vue';
const props = defineProps<{ label: string; inputId?: string; help?: string; error?: string }>();
const id = useId();
const labelId = `${id}-label`;
const helpId = `${id}-help`;
const errorId = `${id}-error`;
const describedBy = computed(() => [props.help ? helpId : '', props.error ? errorId : ''].filter(Boolean).join(' ') || undefined);
</script>
<template>
  <div
    class="settings-row"
    :class="{ 'settings-row-invalid': error }"
  >
    <label
      v-if="inputId"
      :id="labelId"
      :for="inputId"
      class="settings-row-label"
    >{{ label }}</label>
    <div
      v-else
      :id="labelId"
      class="settings-row-label"
    >
      {{ label }}
    </div>
    <div
      class="settings-row-control"
      role="group"
      :aria-labelledby="labelId"
      :aria-describedby="describedBy"
      :aria-invalid="error ? true : undefined"
    >
      <slot />
      <p
        v-if="help"
        :id="helpId"
        class="setting-hint"
      >
        {{ help }}
      </p>
      <p
        v-if="error"
        :id="errorId"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
    </div>
  </div>
</template>
<style scoped>
.settings-row{display:grid;grid-template-columns:minmax(120px,170px) minmax(0,1fr);gap:var(--ui-space-5,24px);align-items:start;padding:var(--ui-space-3,12px) 0;border-bottom:1px solid var(--ui-border);font-size:var(--ui-text-body,14px);line-height:var(--ui-leading-body,1.6)}
.settings-row-label{min-width:0;padding-top:calc((var(--ui-control-height,40px) - var(--ui-leading-label) * var(--ui-text-label,14px)) / 2);color:var(--ink);font-size:var(--ui-text-label,14px);line-height:var(--ui-leading-label);font-weight:var(--ui-weight-medium,500);overflow-wrap:anywhere}
.settings-row-control{min-width:0}
.settings-row-control :deep(input:not([type=checkbox],[type=radio],[type=range],[type=hidden],[type=button],[type=submit],[type=reset],[type=file],[type=color],.numeric-slider-input,.search-field-input,.validated-field-input)),.settings-row-control :deep(select),.settings-row-control :deep(textarea){width:100%;min-width:0;min-height:var(--ui-control-height,40px);box-sizing:border-box;padding:max(var(--ui-space-1),calc((var(--ui-control-height) - var(--ui-text-label) * var(--ui-leading-control) - 2px) / 2)) var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md,8px);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:var(--ui-text-label,14px);line-height:var(--ui-leading-control)}
.settings-row-control :deep(input[type=number]){font-variant-numeric:tabular-nums}
.settings-row-control :deep(textarea){resize:vertical;min-height:calc(2 * var(--ui-control-height,40px))}
.settings-row-control :deep(input:focus-visible),.settings-row-control :deep(select:focus-visible),.settings-row-control :deep(textarea:focus-visible),.settings-row-control :deep(button:focus-visible){outline:var(--ui-focus-ring,2px solid var(--ui-focus));outline-offset:var(--ui-focus-offset,3px)}
.settings-row-control :deep(input:disabled),.settings-row-control :deep(select:disabled),.settings-row-control :deep(textarea:disabled),.settings-row-control :deep(button:disabled){opacity:.55;cursor:not-allowed}
.settings-row-invalid .settings-row-control :deep(input:not([type=checkbox],[type=radio],[type=range])),.settings-row-invalid .settings-row-control :deep(select),.settings-row-invalid .settings-row-control :deep(textarea){border-color:var(--ui-danger)}
.settings-row-control :deep(.setting-hint),.settings-row-control :deep(.settings-error){margin:var(--ui-space-2,8px) 0 0;font-size:var(--ui-text-caption,12px);line-height:var(--ui-leading-caption);overflow-wrap:anywhere}
.settings-row-control :deep(.setting-hint){color:var(--muted)}
.settings-row-control :deep(.settings-error){color:var(--ui-danger)}
.settings-row-control :deep(button){font-family:inherit;font-size:var(--ui-text-label,14px)}
.settings-row-control :deep(.java-path),.settings-row-control :deep(.runtime-java-path){font-family:var(--ui-font-mono,monospace);font-size:var(--ui-text-caption,12px);overflow-wrap:anywhere}
@media(max-width:600px){.settings-row{grid-template-columns:1fr;gap:var(--ui-space-2,8px)}.settings-row-label{padding-top:0}}
</style>
