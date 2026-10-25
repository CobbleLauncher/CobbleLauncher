// SPDX-License-Identifier: MIT
import {mount,flushPromises} from '@vue/test-utils';
import {beforeEach,afterEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import PluginsPanel from './PluginsPanel.vue';
import PluginCatalogsPanel from './PluginCatalogsPanel.vue';
import PluginPermissions from './PluginPermissions.vue';
import {addedPermissionNodes,effectivePermissionNodes} from './plugin-permissions';
import type {CatalogManifest} from './plugin-catalogs';
import {permissionCategory,permissionTone,requiresSensitiveConsent} from './extensions/permission-review';
import {setLocale} from './i18n';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const old:CatalogManifest={id:'composer',name:'Fixture composer',version:'1',api_version:'0.1.0',publisher:'Fixture',component_sha256:'a'.repeat(64),capabilities:['message-composer'],file_extensions:[],composer_permissions:['draft']};
const expanded:CatalogManifest={...old,version:'2',permissions:[{permission:'launcher:composer/draft'},{permission:'launcher:composer/context/selected-session'}],composer_permissions:['draft','selected_session']};
const stubs={PluginCatalogsPanel:true,BundledPluginsPanel:true,PluginIdentity:true};
beforeEach(()=>{setLocale('en-US');vi.mocked(invoke).mockReset();vi.mocked(invoke).mockImplementation(async command=>command==='list_plugins'?[{manifest:old,enabled:true}]:command==='get_pending_plugin_signature'?{status:'unsigned'}:[]);});
afterEach(()=>setLocale('zh-CN'));
async function review(manifest:CatalogManifest,previous?:CatalogManifest){const wrapper=mount(PluginsPanel,{global:{stubs}});await flushPromises();wrapper.findComponent(PluginCatalogsPanel).vm.$emit('prepared',{manifest,sourceName:'Fixture catalog',...(previous?{updateFrom:previous}:{})});await flushPromises();return wrapper;}
it('requires acknowledgement only for concrete additional access and submits the full reviewed manifest',async()=>{
 const wrapper=await review(expanded,old);expect(wrapper.get('.plugin-permission-expansion').text()).toContain('selected game’s name, version and loader');
 const allow=wrapper.get('.plugin-approval .primary-button');expect(allow.attributes('disabled')).toBeDefined();await allow.trigger('click');expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='update_plugin')).toBe(false);
 await wrapper.get('.plugin-permission-expansion input').setValue(true);await allow.trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('update_plugin',{id:expanded.id,reviewedManifest:expanded});wrapper.unmount();
});
it('does not add an acknowledgement to an unchanged permission update',async()=>{
 const next={...old,version:'2'};const wrapper=await review(next,old);expect(wrapper.find('.plugin-permission-expansion').exists()).toBe(false);expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeUndefined();
 await wrapper.get('.plugin-approval .primary-button').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('update_plugin',{id:next.id,reviewedManifest:next});wrapper.unmount();
});
it('describes snapshot history and host restore confirmation in both locales',async()=>{
 const manifest:CatalogManifest={...old,permissions:[{permission:'launcher:world-snapshots/read'},{permission:'launcher:world-snapshots/restore-request'}]};
 const wrapper=mount(PluginPermissions,{props:{manifest}});
 expect(wrapper.text()).toContain('Read world snapshot history and file changes');
 expect(wrapper.text()).toContain("Open the launcher's restore confirmation");
 setLocale('zh-CN');await flushPromises();expect(wrapper.text()).toContain('读取世界快照历史和文件变更');
 expect(wrapper.text()).toContain('打开启动器的世界快照恢复确认界面');
 wrapper.unmount();
});
it('resets the acknowledgement when a different package is prepared',async()=>{
 const wrapper=await review(expanded,old);await wrapper.get('input[type="checkbox"]').setValue(true);
 wrapper.findComponent(PluginCatalogsPanel).vm.$emit('prepared',{manifest:{...expanded,version:'3'},sourceName:'Fixture catalog',updateFrom:old});await flushPromises();
 expect(wrapper.get<HTMLInputElement>('input[type="checkbox"]').element.checked).toBe(false);expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeDefined();wrapper.unmount();
});
it('uses the same scope review for a local package updating an installed extension',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>command==='list_plugins'?[{manifest:old,enabled:true}]:command==='preview_plugin_package'?expanded:command==='get_pending_plugin_signature'?{status:'unsigned'}:[]);
 const wrapper=mount(PluginsPanel,{global:{stubs}});await flushPromises();await wrapper.findAll('button').find(button=>button.text()==='Install local plugin')!.trigger('click');await flushPromises();
 expect(wrapper.find('.plugin-permission-expansion').exists()).toBe(true);expect(wrapper.get('.plugin-approval .primary-button').text()).toBe('Allow update');wrapper.unmount();
});
it('shows selected resources and localized settings scope without exposing permission node IDs',()=>{
 const report:CatalogManifest={...old,api_version:'0.2.0',capabilities:['report'],file_extensions:['log'],composer_permissions:[],contributions:{schema_version:1,settings:{categories:[{id:'report',fields:[{id:'page_size',scope:'global',label:{en:'Page size','zh-CN':'每页数量'}}]}]}}};
 const wrapper=mount(PluginPermissions,{props:{manifest:report}});expect(wrapper.text()).toContain('Read selected .log files');expect(wrapper.text()).toContain('Page size (global)');expect(wrapper.text()).not.toContain('launcher:settings/read');wrapper.unmount();
});
it('compares explicit and legacy nodes by contained scopes, without prompting for reductions or reordering',()=>{
 const provider:CatalogManifest={...old,capabilities:['blueprint-provider'],composer_permissions:[],provider_origins:['https://one.example','https://two.example']};
 expect(addedPermissionNodes({...provider,permissions:[{permission:'launcher:provider/https/get',origins:['https://two.example']}]},provider)).toEqual([]);
 expect(addedPermissionNodes({...provider,provider_origins:['https://two.example','https://one.example']},provider)).toEqual([]);
 expect(addedPermissionNodes({...provider,provider_origins:[...provider.provider_origins!,'https://three.example']},provider)).toEqual([{permission:'launcher:provider/https/get',origins:['https://three.example']}]);
 expect(effectivePermissionNodes(expanded)).toEqual(expanded.permissions);
});

it('reviews concrete provider origins on first install and binds installation to that manifest',async()=>{
 const manifest:CatalogManifest={...old,id:'provider',api_version:'0.2.0',capabilities:['blueprint-provider'],composer_permissions:[],provider_origins:['https://catalog.example'],permissions:[{permission:'launcher:provider/https/get',origins:['https://catalog.example']}]};
 const wrapper=await review(manifest);expect(wrapper.get('.plugin-approval').text()).toContain('Search and download from https://catalog.example');
 expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeDefined();
 await wrapper.get('.plugin-approval .primary-button').trigger('click');
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='install_plugin')).toBe(false);
 await wrapper.get('.sensitive-permissions input').setValue(true);
 await wrapper.get('.plugin-approval .primary-button').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('install_plugin',{id:manifest.id,reviewedManifest:manifest});wrapper.unmount();
});
it('requires each sensitive scope separately and resets consent for the next package',async()=>{
 const manifest:CatalogManifest={...old,id:'config-tool',permissions:[{permission:'launcher:game-config/read'},{permission:'launcher:ui/webview'}]};
 const wrapper=await review(manifest);const checks=wrapper.findAll('.sensitive-permissions input');expect(checks).toHaveLength(2);
 await checks[0]!.setValue(true);expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeDefined();
 await checks[1]!.setValue(true);expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeUndefined();
 wrapper.findComponent(PluginCatalogsPanel).vm.$emit('prepared',{manifest:{...manifest,version:'3'},sourceName:'Fixture catalog'});await flushPromises();
 expect(wrapper.findAll<HTMLInputElement>('.sensitive-permissions input').every(input=>!input.element.checked)).toBe(true);wrapper.unmount();
});
it('only asks for newly added network origins on an update',async()=>{
 const previous:CatalogManifest={...old,permissions:[{permission:'launcher:provider/https/get',origins:['https://one.example']}]};
 const next:CatalogManifest={...previous,version:'2',permissions:[{permission:'launcher:provider/https/get',origins:['https://one.example','https://two.example']}]};
 const wrapper=await review(next,previous);const scopes=wrapper.get('.sensitive-permissions');
 expect(scopes.text()).toContain('https://two.example');expect(scopes.text()).not.toContain('https://one.example');
 await scopes.get('input').setValue(true);await wrapper.get('.plugin-approval .primary-button').trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('update_plugin',{id:next.id,reviewedManifest:next});wrapper.unmount();
});
it('keeps a rejected manifest review visible and explains why it must be repeated',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>{if(command==='list_plugins')return[{manifest:old,enabled:true}];if(command==='get_pending_plugin_signature')return{status:'unsigned'};if(command==='update_plugin')throw 'PLUGIN_REVIEW_MISMATCH: pinned package differs';return[];});
 const wrapper=await review(expanded,old);await wrapper.get('input[type="checkbox"]').setValue(true);await wrapper.get('.plugin-approval .primary-button').trigger('click');await flushPromises();
 expect(wrapper.text()).toContain('The package changed. Review it again');expect(wrapper.find('.plugin-approval').exists()).toBe(true);expect(wrapper.text()).not.toContain('PLUGIN_REVIEW_MISMATCH');wrapper.unmount();
});
it('recognizes additional settings keys as scoped access rather than a capability label change',()=>{
 const report:CatalogManifest={...old,api_version:'0.2.0',capabilities:['report'],file_extensions:['log'],composer_permissions:[],contributions:{schema_version:1,settings:{categories:[{id:'report',fields:[{id:'page_size',scope:'global'}]}]}}};
 const next:CatalogManifest={...report,contributions:{schema_version:1,settings:{categories:[{id:'report',fields:[{id:'page_size',scope:'global'},{id:'show_dependencies',scope:'instance'}]}]}}};
 expect(addedPermissionNodes(next,report)).toEqual([{permission:'launcher:settings/read',keys:['instance:show_dependencies']}]);
});

it('treats selected-folder access as additional to selected-file access and renders its concrete file scope',()=>{
 const before:CatalogManifest={...old,api_version:'0.2.0',capabilities:['report'],composer_permissions:[],file_extensions:['json'],permissions:[{permission:'launcher:resource/selected/read',extensions:['json']}]};
 const folder={permission:'launcher:resource/selected-folder/read' as const,extensions:['json']};const next:CatalogManifest={...before,report_protocols:['cobble.config-folder@1'],permissions:[...before.permissions!,folder]};expect(addedPermissionNodes(next,before)).toEqual([folder]);const wrapper=mount(PluginPermissions,{props:{manifest:next,nodes:[folder]}});expect(wrapper.text()).toContain('.json configuration files in the folder you select');wrapper.unmount();
});

it('reviews saved-world seed access as a sensitive, narrowly scoped game permission',async()=>{
 const seed={permission:'launcher:worlds/seed/read' as const};
 const previous:CatalogManifest={...old,id:'seed-compass',permissions:[{permission:'launcher:game-inspection/read',scopes:['worlds']}]};
 const next:CatalogManifest={...previous,version:'2',permissions:[...previous.permissions!,seed]};
 expect(addedPermissionNodes(next,previous)).toEqual([seed]);
 expect(requiresSensitiveConsent(seed)).toBe(true);
 expect(permissionTone(seed)).toBe('files');
 expect(permissionCategory(seed)).toBe('game');
 const wrapper=await review(next,previous);
 const consent=wrapper.get('.sensitive-permission');
 expect(consent.text()).toContain('Read the selected world’s seed and generation details');
 expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeDefined();
 await consent.get('input').setValue(true);
 expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeUndefined();
 setLocale('zh-CN');await flushPromises();
 expect(consent.text()).toContain('读取所选存档的种子和生成信息');
 wrapper.unmount();setLocale('en-US');
});

it('requires first-install consent for world inventory and seed access',async()=>{
 const manifest:CatalogManifest={...old,id:'seed-compass',permissions:[
  {permission:'launcher:game-inspection/read',scopes:['worlds']},
  {permission:'launcher:worlds/seed/read'},
 ]};
 const wrapper=await review(manifest);
 const consents=wrapper.findAll('.sensitive-permission');
 expect(consents).toHaveLength(2);
 const seedConsent=consents.find(item=>item.text().includes('Read the selected world’s seed and generation details'))!;
 expect(seedConsent.exists()).toBe(true);
 expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeDefined();
 for(const consent of consents) await consent.get('input').setValue(true);
 expect(wrapper.get('.plugin-approval .primary-button').attributes('disabled')).toBeUndefined();
 wrapper.unmount();
});

it('reviews AI network access separately and shows useful labels', () => {
 const nodes = [{permission:'launcher:ai/chat' as const},{permission:'launcher:ai/configure' as const}];
 for(const node of nodes){expect(requiresSensitiveConsent(node)).toBe(true);expect(permissionCategory(node)).toBe('network');expect(permissionTone(node)).toBe('network');}
 const wrapper=mount(PluginPermissions,{props:{manifest:{...old,permissions:nodes}}});
 expect(wrapper.text()).toContain('Send chat messages to the AI service you configure');
 expect(wrapper.text()).toContain('Set the AI service address, model and credential');
 expect(wrapper.text()).not.toContain('launcher:ai');wrapper.unmount();
});
