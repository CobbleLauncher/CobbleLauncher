// SPDX-License-Identifier: MIT
import { effectScope, ref } from 'vue';
import { flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useMeshContactsPreview } from './use-mesh-contacts-preview';
import type { MeshAppSnapshot } from './mesh-social';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const scopes: ReturnType<typeof effectScope>[] = [];
const snapshot = (name='River', connected=true): MeshAppSnapshot => ({ profile:{peer_id:'self',display_name:'Dylan',suffix:'1234',revision:1}, contacts:[{profile:{peer_id:name,display_name:name,suffix:'5678',revision:1},connected,last_seen:42}],capabilities:{contact_invites:true,direct_messages:true,lan_only:true,relay_messages:false,groups:false,vlan:false} });
function start() { const opened=ref(false),available=ref(true),scope=effectScope();scopes.push(scope);return {opened,available,preview:scope.run(()=>useMeshContactsPreview({open:opened,available}))!}; }
function visibility(state: DocumentVisibilityState) { Object.defineProperty(document,'visibilityState',{configurable:true,value:state});document.dispatchEvent(new Event('visibilitychange')); }
beforeEach(()=>{vi.useFakeTimers();vi.mocked(invoke).mockReset().mockResolvedValue(snapshot());visibility('visible');});
afterEach(()=>{scopes.splice(0).forEach(scope=>scope.stop());vi.useRealTimers();visibility('visible');});
it('does no first-screen IPC, polls only while open and stops when closed',async()=>{
  const view=start();await flushPromises();expect(invoke).not.toHaveBeenCalled();view.opened.value=true;await flushPromises();expect(invoke).toHaveBeenCalledExactlyOnceWith('mesh_app_snapshot');expect(view.preview.preview.value[0]?.connected).toBe(true);
  await vi.advanceTimersByTimeAsync(3000);expect(invoke).toHaveBeenCalledTimes(2);view.opened.value=false;expect(view.preview.preview.value).toEqual([]);await vi.advanceTimersByTimeAsync(9000);expect(invoke).toHaveBeenCalledTimes(2);
});
it('clears state while hidden, resumes when visible and stops after disposal',async()=>{
  const view=start();view.opened.value=true;await flushPromises();visibility('hidden');expect(view.preview.preview.value).toEqual([]);await vi.advanceTimersByTimeAsync(6000);expect(invoke).toHaveBeenCalledTimes(1);visibility('visible');await flushPromises();expect(invoke).toHaveBeenCalledTimes(2);scopes[0]!.stop();await vi.advanceTimersByTimeAsync(6000);expect(invoke).toHaveBeenCalledTimes(2);
});
it('clears stale connected data on failure and retries only the read',async()=>{
  const view=start();view.opened.value=true;await flushPromises();vi.mocked(invoke).mockRejectedValueOnce(Error('private details'));await vi.advanceTimersByTimeAsync(3000);expect(view.preview.state.value).toBe('error');expect(view.preview.preview.value).toEqual([]);await view.preview.refresh();expect(view.preview.state.value).toBe('ready');expect(vi.mocked(invoke).mock.calls.every(([command])=>command==='mesh_app_snapshot')).toBe(true);
});
it('ignores a read from a previous open and an unavailable context',async()=>{
  let finish!: (value:MeshAppSnapshot)=>void;vi.mocked(invoke).mockReturnValueOnce(new Promise(resolve=>{finish=resolve;}));const view=start();view.opened.value=true;view.opened.value=false;view.opened.value=true;await flushPromises();finish(snapshot('Old'));await flushPromises();expect(view.preview.preview.value[0]?.profile.display_name).toBe('River');view.available.value=false;expect(view.preview.preview.value).toEqual([]);await vi.advanceTimersByTimeAsync(6000);expect(invoke).toHaveBeenCalledTimes(2);
});
it('orders connected contacts first and limits the preview to eight saved contacts',async()=>{
  const value=snapshot();value.contacts=Array.from({length:12},(_,index)=>({profile:{peer_id:String(index),display_name:'Contact '+String(index).padStart(2,'0'),suffix:'1234',revision:1},connected:index===11,last_seen:42}));vi.mocked(invoke).mockResolvedValue(value);const view=start();view.opened.value=true;await flushPromises();expect(view.preview.preview.value).toHaveLength(8);expect(view.preview.preview.value.map(contact=>contact.profile.peer_id)).toEqual(['11','0','1','2','3','4','5','6']);
});
