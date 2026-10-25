// SPDX-License-Identifier: MIT
import { computed, onScopeDispose, ref, shallowRef, toValue, watch, type MaybeRefOrGetter } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale } from './i18n';
import type { MeshAppSnapshot, MeshContact } from './mesh-social';

export function useMeshContactsPreview(options: { open: MaybeRefOrGetter<boolean>; available: MaybeRefOrGetter<boolean> }) {
  const contacts = shallowRef<MeshContact[]>([]);
  const state = ref<'idle' | 'loading' | 'ready' | 'error'>('idle');
  const refreshing = ref(false);
  const visible = ref(document.visibilityState !== 'hidden');
  const active = () => toValue(options.open) && toValue(options.available) && visible.value;
  let disposed = false, revision = 0, timer: ReturnType<typeof setTimeout> | undefined;
  function stop() {
    ++revision;
    if (timer) clearTimeout(timer);
    timer = undefined;
    contacts.value = []; state.value = 'idle'; refreshing.value = false;
  }
  async function refresh() {
    if (disposed || !active() || refreshing.value) return;
    if (timer) clearTimeout(timer);
    timer = undefined;
    const current = revision;
    refreshing.value = true;
    if (state.value !== 'ready') state.value = 'loading';
    try {
      const value = await invoke<MeshAppSnapshot | null>('mesh_app_snapshot');
      if (disposed || current !== revision || !active()) return;
      contacts.value = value?.contacts ?? [];
      state.value = 'ready';
    } catch {
      if (disposed || current !== revision || !active()) return;
      contacts.value = []; state.value = 'error';
    } finally {
      if (!disposed && current === revision && active()) {
        refreshing.value = false;
        timer = setTimeout(() => { void refresh(); }, 3000);
      }
    }
  }
  function visibilityChanged() { visible.value = document.visibilityState !== 'hidden'; }
  document.addEventListener('visibilitychange', visibilityChanged);
  watch(() => active(), enabled => { stop(); if (enabled) void refresh(); }, { immediate: true, flush: 'sync' });
  onScopeDispose(() => { disposed = true; stop(); document.removeEventListener('visibilitychange', visibilityChanged); });
  const preview = computed(() => [...contacts.value].sort((a,b) => Number(b.connected) - Number(a.connected) || a.profile.display_name.localeCompare(b.profile.display_name, locale.value) || a.profile.peer_id.localeCompare(b.profile.peer_id)).slice(0,8));
  return { preview, state, refreshing, refresh };
}
