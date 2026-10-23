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
.switch-field{display:inline-flex;align-items:center;flex:none;width:40px;height:24px;padding:3px;border:1px solid var(--ui-border-strong);border-radius:20px;background:var(--ui-surface-3);transition:background var(--ui-duration),border-color var(--ui-duration)}
.switch-field[data-state=checked]{background:var(--ui-accent);border-color:var(--ui-accent)}
.switch-field-thumb{display:block;width:16px;height:16px;border-radius:50%;background:var(--ui-text-muted);transform:translateX(0);transition:transform var(--ui-duration),background var(--ui-duration)}
.switch-field[data-state=checked] .switch-field-thumb{background:var(--ui-on-accent);transform:translateX(14px)}
.switch-field:disabled{opacity:.5;cursor:not-allowed}
@media(prefers-reduced-motion:reduce){.switch-field,.switch-field-thumb{transition:none}}
</style>
