<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref, watch } from 'vue';
import { CollapsibleRoot, CollapsibleTrigger, CollapsibleContent } from 'reka-ui';
import AppIcon from '../AppIcon.vue';
const props = defineProps<{ title: string; bordered?: boolean; defaultOpen?: boolean }>();
const open = ref(props.defaultOpen ?? false);
const emit = defineEmits<{ 'update:open': [value: boolean] }>();
watch(open, value => emit('update:open', value));
</script>
<template>
  <CollapsibleRoot
    v-model:open="open"
    class="settings-disclosure"
    :class="{ 'settings-disclosure-card': bordered }"
  >
    <CollapsibleTrigger
      type="button"
      class="settings-disclosure-trigger"
    >
      <span>{{ title }}</span><AppIcon
        name="chevron-down"
        class="ui-disclosure-icon"
      />
    </CollapsibleTrigger>
    <CollapsibleContent
      v-show="open"
      force-mount
      class="settings-disclosure-content"
    >
      <slot />
    </CollapsibleContent>
  </CollapsibleRoot>
</template>
<style scoped>
.settings-disclosure{min-width:0;margin-top:var(--ui-space-3,12px)}
.settings-disclosure-card{margin:var(--ui-space-4,16px) 0;border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg,12px);background:var(--ui-surface-1);padding:0 var(--ui-space-4,16px)}
.settings-disclosure-trigger{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3,12px);width:100%;min-height:var(--ui-control-height,40px);padding:var(--ui-space-2,8px) 0;border:0;border-radius:var(--ui-radius-sm,6px);background:transparent;color:var(--ink);font:inherit;font-size:var(--ui-text-label,13px);line-height:var(--ui-leading-body,1.6);font-weight:var(--ui-weight-medium,500);text-align:left;cursor:pointer}
.settings-disclosure-trigger:focus-visible{outline:var(--ui-focus-ring,2px solid var(--ui-focus));outline-offset:var(--ui-focus-offset,3px)}
.settings-disclosure-trigger:disabled{opacity:.55;cursor:not-allowed}
.settings-disclosure-trigger>span{min-width:0;overflow-wrap:anywhere}
.settings-disclosure-trigger>svg{width:var(--ui-icon-sm,16px);height:var(--ui-icon-sm,16px);flex:none}
.settings-disclosure-trigger[data-state="open"]>svg{transform:rotate(180deg)}
.settings-disclosure-card>.settings-disclosure-trigger{min-height:calc(var(--ui-control-height,40px) + var(--ui-space-2,8px));font-weight:var(--ui-weight-semibold,600)}
.settings-disclosure-content{min-width:0;padding-bottom:var(--ui-space-1,4px)}
.settings-disclosure-card :deep(.settings-row:last-of-type){border-bottom:0}
@media(max-width:600px){.settings-disclosure-card{padding:0 var(--ui-space-3,12px)}}
</style>
