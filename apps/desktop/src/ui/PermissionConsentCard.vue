<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { Check, ShieldCheck } from '@lucide/vue';
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
    <ShieldCheck
      v-if="checked"
      class="permission-consent-approved"
      :size="18"
      aria-hidden="true"
    />
  </label>
</template>
<style scoped>
.permission-consent{--consent-tone:var(--muted);position:relative;display:flex;align-items:center;gap:12px;min-height:56px;padding:12px 14px;border:0;border-left:3px solid color-mix(in srgb,var(--consent-tone) 55%,transparent);border-radius:8px;background:color-mix(in srgb,var(--consent-tone) 6%,var(--panel));cursor:pointer;transition:background .15s;font-size:14px;line-height:1.5}
.permission-consent[data-tone=network]{--consent-tone:var(--ui-warning)}.permission-consent[data-tone=files]{--consent-tone:var(--ui-info)}.permission-consent[data-tone=write]{--consent-tone:var(--ui-danger)}
.permission-consent:hover{background:color-mix(in srgb,var(--consent-tone) 10%,var(--panel))}
.permission-consent.is-checked{border-left-color:var(--consent-tone);background:color-mix(in srgb,var(--consent-tone) 10%,var(--panel))}
.permission-consent input{position:absolute;width:1px;height:1px;opacity:0}
.permission-consent:has(input:focus-visible){outline:var(--ui-focus-ring,2px solid var(--ui-focus));outline-offset:3px}
.permission-consent-check{display:grid;place-items:center;flex:none;width:20px;height:20px;border:1.5px solid var(--muted);border-radius:6px;background:var(--panel)}
.is-checked .permission-consent-check{background:var(--accent);border-color:var(--accent);color:var(--accent-ink)}
.permission-consent-content{flex:1;min-width:0;overflow-wrap:anywhere}.permission-consent-approved{flex:none;color:var(--accent)}
.is-disabled{opacity:.55;cursor:default}
@media(prefers-reduced-motion:reduce){.permission-consent{transition:none}}
</style>
