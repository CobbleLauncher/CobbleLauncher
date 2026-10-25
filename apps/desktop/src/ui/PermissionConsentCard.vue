<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { Check } from '@lucide/vue';
import { useId } from 'vue';
import type { PermissionTone } from '../extensions/permission-review';
const contentId = useId();
withDefaults(defineProps<{ label: string; disabled?: boolean; tone?: PermissionTone }>(), { tone: 'neutral' });
const checked = defineModel<boolean>({ required: true });
</script>
<template>
  <label
    class="permission-consent"
    :class="{ 'is-checked': checked, 'is-disabled': disabled }"
    :data-tone="tone"
  >
    <input
      v-model="checked"
      type="checkbox"
      :disabled="disabled"
      :aria-labelledby="contentId"
    >
    <span
      class="permission-consent-check"
      aria-hidden="true"
    ><Check
      v-if="checked"
      :size="14"
      :stroke-width="3"
    /></span>
    <span
      :id="contentId"
      class="permission-consent-content"
    ><slot><span>{{ label }}</span></slot></span>
  </label>
</template>
<style scoped>
.permission-consent{position:relative;display:flex;align-items:center;gap:var(--ui-space-3);min-height:var(--ui-control-height);padding:var(--ui-space-3) var(--ui-space-4);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);cursor:pointer;transition:background-color var(--ui-duration-fast),border-color var(--ui-duration-fast);font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.permission-consent:hover:not(.is-disabled){background:var(--ui-surface-hover)}.permission-consent.is-checked{border-color:var(--ui-focus);background:var(--ui-surface-2)}.permission-consent input{position:absolute;width:1px;height:1px;opacity:0}.permission-consent:has(input:focus-visible){outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}.permission-consent-check{display:grid;place-items:center;flex:none;width:20px;height:20px;border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-sm);background:var(--ui-surface-1)}.is-checked .permission-consent-check{background:var(--ui-accent);border-color:var(--ui-focus);color:var(--ui-on-accent)}.permission-consent-content{flex:1;min-width:0;overflow-wrap:anywhere}.permission-consent.is-disabled{opacity:.55;cursor:not-allowed}
</style>
