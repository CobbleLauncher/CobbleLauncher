<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from 'reka-ui';
defineProps<{ id?: string; label: string; disabled?: boolean }>();
const enabled = defineModel<boolean>({ required: true });
</script>
<template>
  <SwitchRoot
    :id="id"
    v-model="enabled"
    :disabled="disabled"
    :aria-label="label"
    class="switch-field"
  >
    <SwitchThumb class="switch-field-thumb" />
  </SwitchRoot>
</template>
<style scoped>
.switch-field{position:relative;display:inline-flex;align-items:center;flex:none;width:40px;height:var(--ui-control-height);padding:0;border:0;border-radius:var(--ui-radius-sm);background:transparent;cursor:pointer}
.switch-field::before{content:"";position:absolute;inset-inline:0;top:50%;height:24px;transform:translateY(-50%);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-round);background:var(--ui-surface-3);transition:background-color var(--ui-duration-fast),border-color var(--ui-duration-fast)}
.switch-field[data-state=checked]::before{background:var(--ui-accent);border-color:var(--ui-accent)}
.switch-field-thumb{position:relative;display:block;width:16px;height:16px;margin-left:4px;border-radius:var(--ui-radius-round);background:var(--ui-text-muted);transform:translateX(0);transition:transform var(--ui-duration-normal),background-color var(--ui-duration-fast)}
.switch-field[data-state=checked] .switch-field-thumb{background:var(--ui-on-accent);transform:translateX(16px)}
.switch-field:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}
.switch-field:disabled{opacity:.5;cursor:not-allowed}
</style>
