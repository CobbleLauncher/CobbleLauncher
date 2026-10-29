<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { locale } from './i18n';
import { usePackDrag } from './use-pack-drag';
import AppIcon from './ui/AppIcon.vue';
const props = defineProps<{ blocked?: boolean }>();
const drag = usePackDrag();
const message = computed(() => {
  const zh = locale.value === 'zh-CN';
  if (props.blocked) return zh ? '请先完成当前操作' : 'Finish the current action first';
  if (drag.value.kind === 'mrpack' && drag.value.count === 1) return zh ? '拖到这里安装整合包' : 'Drop here to install modpack';
  return zh ? '拖到这里查看文件' : 'Drop here to inspect files';
});
</script>
<template>
  <Transition name="pack-drop">
    <div
      v-if="drag.active"
      class="pack-drop-overlay"
      :class="{ 'is-blocked': blocked }"
      role="status"
      aria-live="polite"
    >
      <div class="pack-drop-target">
        <AppIcon
          name="download"
          :size="48"
        /><strong>{{ message }}</strong>
      </div>
    </div>
  </Transition>
</template>
<style scoped>
.pack-drop-overlay { position: fixed; inset: 0; z-index: 10000; display: grid; place-items: center; padding: var(--ui-space-6); pointer-events: none; background: color-mix(in srgb, var(--ui-bg) 82%, transparent); backdrop-filter: blur(8px); color: var(--ui-text); }
.pack-drop-target { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--ui-space-4); width: min(560px, 100%); min-height: 250px; border: 2px dashed currentColor; border-radius: var(--ui-radius-lg); }
.pack-drop-target strong { font-size: var(--ui-text-page); font-weight: var(--ui-weight-semibold); }
.is-blocked { color: var(--ui-text-muted); }
.pack-drop-enter-active, .pack-drop-leave-active { transition: opacity var(--ui-duration-fast) var(--ui-easing); }
.pack-drop-enter-from, .pack-drop-leave-to { opacity: 0; }
@media (prefers-reduced-motion: reduce) { .pack-drop-enter-active, .pack-drop-leave-active { transition: none; } }
:global([data-motion="reduced"]) .pack-drop-enter-active, :global([data-motion="reduced"]) .pack-drop-leave-active { transition: none; }
</style>
