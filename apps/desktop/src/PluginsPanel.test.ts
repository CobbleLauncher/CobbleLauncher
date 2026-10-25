// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import PluginsPanel from './PluginsPanel.vue';
import PluginCatalogsPanel from './PluginCatalogsPanel.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(async()=>[])}));
it('reviews menu uninstall once before removing the selected extension',async()=>{setLocale('en-US');const plugin={manifest:{id:'reviewed-plugin',name:'Reviewed plugin',version:'1',publisher:'Fixture',api_version:'1.0.0',capabilities:[],file_extensions:[],component_sha256:'0'.repeat(64)},enabled:false};vi.mocked(invoke).mockClear().mockImplementation(async command=>command==='list_plugins'?[plugin]:[]);const wrapper=mount(PluginsPanel);await flushPromises();await wrapper.get('button[aria-label="More actions for Reviewed plugin"]').trigger('click');await flushPromises();const item=[...document.querySelectorAll<HTMLElement>('[role=menuitem]')].find(item=>item.textContent?.trim()==='Uninstall')!;item.click();await flushPromises();expect(invoke).not.toHaveBeenCalledWith('uninstall_plugin',expect.anything());const confirm=document.querySelector<HTMLButtonElement>('.confirmation-confirm')!;expect(confirm.disabled).toBe(false);confirm.click();await flushPromises();expect(invoke).toHaveBeenCalledWith('uninstall_plugin',{id:'reviewed-plugin'});wrapper.unmount();vi.mocked(invoke).mockReset().mockResolvedValue([]);setLocale('zh-CN');});
it('opens the candidate source and plugin without previewing or installing',async()=>{
 const candidate={sourceId:'community-one',pluginId:'optional-provider'};
 const wrapper=mount(PluginsPanel,{props:{candidate}});
 await flushPromises();
 expect(wrapper.get<HTMLInputElement>('input[type="search"]').element.value).toBe(candidate.pluginId);
 expect(wrapper.findComponent(PluginCatalogsPanel).props('candidate')).toEqual(candidate);
 expect(wrapper.find('[data-state="active"][value="sources"]').exists() || wrapper.findComponent(PluginCatalogsPanel).isVisible()).toBe(true);
 expect(vi.mocked(invoke).mock.calls.some(([command])=>/^(preview|install|update|uninstall)_/.test(command))).toBe(false);
 wrapper.unmount();
});

it('filters bundled recommendations in Discover without starting an install', async () => {
 setLocale('en-US');
 const candidate={sourceId:'bundled',pluginId:'seed-compass'};
 vi.mocked(invoke).mockClear();
 const wrapper=mount(PluginsPanel,{props:{candidate}});
 await flushPromises();
 expect(wrapper.get<HTMLInputElement>('input[type="search"]').element.value).toBe(candidate.pluginId);
 expect(wrapper.get('[role="tab"][aria-selected="true"]').text()).toContain('Discover');
 expect(wrapper.findComponent(PluginCatalogsPanel).props('candidate')).toBeUndefined();
 expect(vi.mocked(invoke).mock.calls.some(([command])=>['install_plugin','preview_plugin_package'].includes(command))).toBe(false);
 wrapper.unmount();
 setLocale('zh-CN');
});

it('describes missing capability without calling it corruption and allows disabling the consumer',async()=>{
 setLocale('en-US');
 const plugin={manifest:{id:'consumer',name:'Consumer',version:'1.0.0',publisher:'Fixture',api_version:'1.0.0',capabilities:['viewer'],file_extensions:['schem'],component_sha256:'0'.repeat(64)},enabled:true,problem:'PLUGIN_MISSING_INTERFACE'};
 vi.mocked(invoke).mockImplementation(async command=>command==='list_plugins'?[plugin]:[]);
 const wrapper=mount(PluginsPanel);
 await flushPromises();
 expect(wrapper.text()).toContain('A required plugin capability is unavailable');
 expect(wrapper.text()).not.toContain('Plugin files are damaged');
 const disable=wrapper.get('[role=switch]');
 expect(disable.attributes('disabled')).toBeUndefined();
 expect(disable.attributes('aria-checked')).toBe('true');
 await disable.trigger('click');
 await flushPromises();
 expect(invoke).toHaveBeenCalledWith('set_plugin_enabled',{id:'consumer',enabled:false});
 wrapper.unmount();
 vi.mocked(invoke).mockImplementation(async()=>[]);
 setLocale('zh-CN');
});

it('keeps installed plugin details and removal in both action menus', async () => {
 setLocale('en-US');
 const plugin={manifest:{id:'menu-plugin',name:'Menu Plugin',version:'1.0.0',publisher:'Fixture',api_version:'1.0.0',capabilities:[],file_extensions:[],component_sha256:'0'.repeat(64)},enabled:false};
 vi.mocked(invoke).mockImplementation(async command=>command==='list_plugins'?[plugin]:[]);
 const wrapper=mount(PluginsPanel); await flushPromises();
 expect(wrapper.get('.plugin-list .plugin-identity-button').attributes('aria-label')).toContain('Menu Plugin');
 const actions=wrapper.get('.plugin-row-actions');
 expect(actions.element.lastElementChild?.getAttribute('role')).toBe('switch');
 await actions.get('button[aria-label="More actions for Menu Plugin"]').trigger('click');
 await flushPromises();
 expect(document.body.textContent).toContain('Details');
 expect(document.body.textContent).toContain('Uninstall');
 await wrapper.get('.plugin-row-identity').trigger('contextmenu', { clientX: 20, clientY: 20 });
 await flushPromises();
 expect(document.body.textContent).toContain('Details');
 expect(document.body.textContent).toContain('Uninstall');
 wrapper.unmount();
 vi.mocked(invoke).mockImplementation(async()=>[]);
 setLocale('zh-CN');
});

