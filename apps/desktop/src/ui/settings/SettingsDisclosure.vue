<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref, useId, watch } from 'vue';
import { CollapsibleRoot, CollapsibleTrigger, CollapsibleContent } from 'reka-ui';
import AppIcon from '../AppIcon.vue';
import type { AppIconName } from '../icons';
const props = defineProps<{ title: string; icon?: AppIconName; value?: string; bordered?: boolean; defaultOpen?: boolean }>();
const open = ref(props.defaultOpen ?? false);
const motion = ref<HTMLElement | null>(null);
const contentId = useId();
const emit = defineEmits<{ 'update:open': [value: boolean] }>();
watch(open, value => {
  if (!value && motion.value?.contains(document.activeElement)) {
    motion.value.closest('.settings-disclosure')?.querySelector<HTMLButtonElement>('.settings-disclosure-trigger')?.focus();
  }
  emit('update:open', value);
});
</script>
<template>
  <CollapsibleRoot
    v-model:open="open"
    class="settings-disclosure"
    :class="{ 'settings-disclosure-card': bordered, 'settings-disclosure-has-icon': icon || $slots.icon, 'settings-disclosure-has-meta': value || $slots.meta }"
  >
    <CollapsibleTrigger
      type="button"
      class="settings-disclosure-trigger"
      :aria-controls="contentId"
    >
      <span
        v-if="icon || $slots.icon"
        class="settings-disclosure-leading"
        aria-hidden="true"
      ><slot name="icon"><AppIcon
        v-if="icon"
        :name="icon"
      /></slot></span>
      <span class="settings-disclosure-title">{{ title }}</span>
      <span
        v-if="value || $slots.meta"
        class="settings-disclosure-meta"
      ><slot name="meta">{{ value }}</slot></span>
      <AppIcon
        name="chevron-right"
        class="settings-disclosure-chevron"
      />
    </CollapsibleTrigger>
    <CollapsibleContent
      force-mount
      as-child
    >
      <div
        :id="contentId"
        class="settings-disclosure-region"
        :inert="open ? undefined : true"
      >
        <Transition name="disclosure-expand">
          <div
            v-show="open"
            ref="motion"
            class="settings-disclosure-motion"
          >
            <div class="settings-disclosure-clip">
              <div class="settings-disclosure-content">
                <slot />
              </div>
            </div>
          </div>
        </Transition>
      </div>
    </CollapsibleContent>
  </CollapsibleRoot>
</template>
<style scoped>
.settings-disclosure{--disclosure-title-column:1;min-width:0;margin-block:var(--ui-space-3);border-block:1px solid var(--ui-border)}
.settings-disclosure-card{border:1px solid var(--ui-border);border-radius:var(--ui-radius-sm);overflow:hidden}
.settings-disclosure-has-icon{--disclosure-title-column:2}
.settings-disclosure-trigger{display:grid;grid-template-columns:minmax(0,1fr) auto 16px;align-items:center;gap:var(--ui-space-3);width:100%;min-height:calc(var(--ui-control-height) + var(--ui-space-2));padding:var(--ui-space-3);border:0;border-radius:var(--ui-radius-sm);background:transparent;color:var(--ui-text);font:inherit;text-align:start;cursor:pointer;transition:background-color var(--ui-duration-fast)}
.settings-disclosure-has-icon>.settings-disclosure-trigger{grid-template-columns:20px minmax(0,1fr) auto 16px}
.settings-disclosure-trigger:hover{background:var(--ui-surface-hover)}
.settings-disclosure-trigger[data-state=open]{background:var(--ui-surface-2);border-end-start-radius:0;border-end-end-radius:0}
.settings-disclosure-trigger:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}
.settings-disclosure-leading{grid-column:1;grid-row:1;display:grid;place-items:center;width:20px;height:20px;color:var(--ui-text-muted)}
.settings-disclosure-leading :deep(svg){width:20px;height:20px;flex:none}
.settings-disclosure-title{grid-column:var(--disclosure-title-column);grid-row:1;min-width:0;overflow-wrap:anywhere;font-size:var(--ui-text-label);line-height:var(--ui-leading-label);font-weight:var(--ui-weight-semibold)}
.settings-disclosure-meta{grid-column:-3 / -2;grid-row:1;min-width:0;max-width:260px;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-weight:var(--ui-weight-regular);text-align:end;overflow-wrap:anywhere}
.settings-disclosure-chevron{grid-column:-2 / -1;grid-row:1;width:16px;height:16px;color:var(--ui-text-muted);transition:transform var(--ui-duration-normal)}
.settings-disclosure-trigger[data-state=open] .settings-disclosure-chevron{transform:rotate(90deg)}
.settings-disclosure-content{min-width:0;margin:0 var(--ui-space-3) var(--ui-space-3);padding:var(--ui-space-4) var(--ui-space-2) var(--ui-space-1) var(--ui-space-4);border-top:1px solid var(--ui-border);border-inline-start:2px solid var(--ui-border)}
.settings-disclosure-has-icon>.settings-disclosure-region>.settings-disclosure-motion>.settings-disclosure-clip>.settings-disclosure-content{padding-inline-start:var(--ui-space-6)}
.settings-disclosure-motion{display:grid;grid-template-rows:1fr;opacity:1}
.settings-disclosure-clip{min-height:0;overflow:hidden}
.disclosure-expand-enter-active,.disclosure-expand-leave-active{transition:grid-template-rows 180ms var(--ui-easing,ease-out),opacity var(--ui-duration-fast,120ms) var(--ui-easing,ease-out)}
.disclosure-expand-enter-from,.disclosure-expand-leave-to{grid-template-rows:0fr;opacity:0}
.settings-disclosure-content :deep(.settings-row:last-child){border-bottom:0}
.settings-disclosure summary{list-style:none}.settings-disclosure summary::-webkit-details-marker{display:none}
@media(max-width:600px){
  .settings-disclosure-has-meta .settings-disclosure-title{align-self:end}
  .settings-disclosure-has-meta .settings-disclosure-meta{grid-column:var(--disclosure-title-column);grid-row:2;max-width:100%;text-align:start}
  .settings-disclosure-has-meta .settings-disclosure-chevron{grid-row:1 / span 2}
  .settings-disclosure-has-meta .settings-disclosure-trigger{row-gap:var(--ui-space-1)}
  .settings-disclosure-content,.settings-disclosure-has-icon>.settings-disclosure-region>.settings-disclosure-motion>.settings-disclosure-clip>.settings-disclosure-content{padding-inline-start:var(--ui-space-3)}
}
@media(prefers-reduced-motion:reduce){.disclosure-expand-enter-active,.disclosure-expand-leave-active,.settings-disclosure-chevron{transition:none}}
[data-motion="reduced"] .disclosure-expand-enter-active,[data-motion="reduced"] .disclosure-expand-leave-active,[data-motion="reduced"] .settings-disclosure-chevron{transition:none}
</style>
