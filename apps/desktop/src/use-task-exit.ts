// SPDX-License-Identifier: MIT
import { onUnmounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale } from './i18n';

export function useTaskExit(native: boolean, confirmLeave: () => Promise<boolean>) {
  const open = ref(false);
  const busy = ref(false);
  const error = ref('');
  let answer: ((value: boolean) => void) | undefined;
  let disposed = false;
  function finish(value: boolean) { open.value = false; answer?.(value); answer = undefined; }
  async function confirmExit(): Promise<boolean> {
    if (!await confirmLeave()) return false;
    if (!native || await invoke<number>('get_active_task_count') === 0) return true;
    error.value = '';
    open.value = true;
    return new Promise(resolve => { answer = resolve; });
  }
  async function cancelAndExit() {
    if (busy.value) return;
    busy.value = true;
    error.value = '';
    try {
      await invoke('cancel_active_tasks');
      // Workers finish their cleanup before the window is allowed to close.
      while (!disposed && await invoke<number>('get_active_task_count') > 0) {
        await new Promise(resolve => setTimeout(resolve, 250));
      }
      if (!disposed) finish(true);
    } catch {
      error.value = locale.value === 'zh-CN' ? '无法取消任务，请重试。' : 'Could not cancel the tasks. Try again.';
    } finally { busy.value = false; }
  }
  onUnmounted(() => { disposed = true; finish(false); });
  return { open, busy, error, confirmExit, cancelAndExit, keepWaiting: () => { if (!busy.value) finish(false); } };
}
