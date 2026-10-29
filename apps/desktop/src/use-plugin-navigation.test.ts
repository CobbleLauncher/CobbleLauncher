// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { defineComponent, ref } from 'vue';
import { expect, it, vi } from 'vitest';
import { usePluginNavigation } from './use-plugin-navigation';
const handlers = vi.hoisted(() => new Map<string, (e: { payload: unknown }) => void>());
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (name, handler) => { handlers.set(name, handler); return () => handlers.delete(name); }) }));
it('defers a drop behind consent/busy and retains the leave confirmation', async () => {
  const blocked=ref(true), busy=ref(false), open=vi.fn(), leave=vi.fn(async()=>true);
  const options={native:true, loading:ref(false), busy, packBlocked:blocked, hasGame:()=>true, refreshGames:async()=>{}, confirmLeave:leave, hideTasks:vi.fn(), openInbox:vi.fn(), openInboxReview:async()=>{}, openInboxMarket:vi.fn(), openGame:vi.fn(), openGeneratedBlueprint:vi.fn(), openHandoff:vi.fn(), openRecovery:vi.fn(), openPackPreview:open, error:vi.fn()};
  const wrapper=mount(defineComponent({setup(){usePluginNavigation(options);return()=>null;}}));
  try {
    await flushPromises();
    const payload={kind:'mrpack',preview:{preview_id:'one'}};
    handlers.get('native-pack-preview')!({payload});await flushPromises();expect(open).not.toHaveBeenCalled();expect(leave).not.toHaveBeenCalled();
    busy.value=true;blocked.value=false;await flushPromises();expect(open).not.toHaveBeenCalled();
    busy.value=false;await flushPromises();expect(open).toHaveBeenCalledExactlyOnceWith(payload);
    leave.mockResolvedValueOnce(false);handlers.get('native-pack-preview')!({payload:{kind:'curseforge-pack',preview:{preview_id:'two'}}});await flushPromises();expect(open).toHaveBeenCalledTimes(1);
  } finally {wrapper.unmount();}
  expect(handlers.has('native-pack-preview')).toBe(false);
});
