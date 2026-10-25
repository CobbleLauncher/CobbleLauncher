// SPDX-License-Identifier: MIT
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import MeshContactsButton from './MeshContactsButton.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const wrappers:VueWrapper[]=[];
function mountButton(props={}){const wrapper=mount(MeshContactsButton,{props:{available:true,...props},attachTo:document.body});wrappers.push(wrapper);return wrapper;}
function button(label:string){return [...document.querySelectorAll<HTMLButtonElement>('button')].find(button=>button.textContent?.trim()===label)!;}
beforeEach(()=>{setLocale('en-US');vi.mocked(invoke).mockReset().mockResolvedValue({contacts:[{profile:{peer_id:'exact-peer',display_name:'River',suffix:'5678',revision:1},connected:true,last_seen:42}]});});
afterEach(()=>{wrappers.splice(0).forEach(wrapper=>wrapper.unmount());document.body.innerHTML='';setLocale('zh-CN');});
it('renders a neutral icon without IPC and respects the opening guard',async()=>{
  const guard=vi.fn().mockResolvedValue(false),wrapper=mountButton({beforeOpen:guard});await flushPromises();expect(invoke).not.toHaveBeenCalled();expect(wrapper.get('button').text()).toBe('');await wrapper.get('button').trigger('click');await flushPromises();expect(guard).toHaveBeenCalledOnce();expect(invoke).not.toHaveBeenCalled();expect(document.querySelector('.mesh-contacts-preview')).toBeNull();
});
it('shows truthful connection labels and hands the exact contact to the parent',async()=>{
  const wrapper=mountButton();await wrapper.get('button').trigger('click');await flushPromises();await vi.waitFor(()=>expect(document.querySelector('.mesh-contact-row')).not.toBeNull());expect(document.querySelector('.mesh-contact-row')?.textContent).toContain('River#5678');expect(document.querySelector('.mesh-contact-row')?.textContent).toContain('Connected');(document.querySelector('.mesh-contact-row') as HTMLButtonElement).click();await flushPromises();expect(wrapper.emitted('contact')).toEqual([['exact-peer']]);expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual(['mesh_app_snapshot']);
});
it('hands invitation and management actions to the parent without creating or joining anything',async()=>{
  const wrapper=mountButton();await wrapper.get('button').trigger('click');await flushPromises();await vi.waitFor(()=>expect(button('Join by invitation')).toBeDefined());button('Join by invitation').click();await flushPromises();expect(wrapper.emitted('join')).toEqual([[]]);await wrapper.get('button').trigger('click');await flushPromises();await vi.waitFor(()=>expect(button('Manage contacts')).toBeDefined());button('Manage contacts').click();await flushPromises();expect(wrapper.emitted('open')).toEqual([[]]);expect(vi.mocked(invoke).mock.calls.every(([command])=>command==='mesh_app_snapshot')).toBe(true);
});
it('does not open after availability changes while the guard is pending',async()=>{
  let finish!:(allowed:boolean)=>void;const wrapper=mountButton({beforeOpen:()=>new Promise<boolean>(resolve=>{finish=resolve;})});await wrapper.get('button').trigger('click');await wrapper.setProps({available:false});finish(true);await flushPromises();expect(invoke).not.toHaveBeenCalled();expect(wrapper.get('button').attributes('disabled')).toBeDefined();
});
