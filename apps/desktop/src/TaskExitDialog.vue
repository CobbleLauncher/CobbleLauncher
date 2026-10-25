<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle, DialogDescription } from 'reka-ui';
import { locale } from './i18n';
import AppIcon from './ui/AppIcon.vue';
defineProps<{ open: boolean; busy: boolean; error: string }>();
const emit = defineEmits<{ wait: []; cancel: [] }>();
const copy = computed(() => locale.value === 'zh-CN'
  ? { title: '任务仍在进行', detail: '取消任务并退出，或留在圆石等待完成。', wait: '继续等待', cancel: '取消任务并退出', cancelling: '正在取消…' }
  : { title: 'Tasks are still running', detail: 'Cancel the tasks to exit, or keep Cobble open until they finish.', wait: 'Keep waiting', cancel: 'Cancel tasks and exit', cancelling: 'Cancelling…' });
</script>
<template>
  <DialogRoot
    :open="open"
    @update:open="value => { if (!value && !busy) emit('wait'); }"
  >
    <DialogPortal>
      <DialogOverlay class="task-exit-overlay" />
      <DialogContent
        class="task-exit-dialog"
        @escape-key-down="event => { if (busy) event.preventDefault(); }"
        @pointer-down-outside.prevent
      >
        <DialogTitle>{{ copy.title }}</DialogTitle>
        <DialogDescription>{{ copy.detail }}</DialogDescription>
        <p
          v-if="error"
          role="alert"
        >
          {{ error }}
        </p>
        <div class="task-exit-actions">
          <button
            type="button"
            :disabled="busy"
            @click="emit('wait')"
          >
            {{ copy.wait }}
          </button>
          <button
            type="button"
            :disabled="busy"
            @click="emit('cancel')"
          >
            <AppIcon
              v-if="busy"
              name="loader-circle"
            />{{ busy ? copy.cancelling : copy.cancel }}
          </button>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
<style scoped>
.task-exit-overlay{position:fixed;inset:0;z-index:1100;background:#0007}.task-exit-dialog{position:fixed;z-index:1101;left:50%;top:50%;transform:translate(-50%,-50%);width:min(440px,calc(100vw - 32px));box-sizing:border-box;padding:24px;border-radius:var(--radius,12px);background:var(--surface);color:var(--ink);box-shadow:0 16px 50px #0003}.task-exit-dialog h2{margin:0;font-size:18px}.task-exit-dialog p{color:var(--muted);line-height:1.5}.task-exit-actions{display:flex;justify-content:flex-end;gap:10px;flex-wrap:wrap;margin-top:24px}.task-exit-actions button{display:inline-flex;align-items:center;gap:8px;padding:10px 14px;border:1px solid var(--line);border-radius:8px;background:var(--surface);color:var(--ink);font:inherit}.task-exit-actions button:last-child{background:var(--ink);color:var(--surface)}
</style>
