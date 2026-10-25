// SPDX-License-Identifier: MIT
import {mount,flushPromises} from '@vue/test-utils';
import {beforeEach,expect,it,vi} from 'vitest';
import MaterialPlannerButton from './MaterialPlannerButton.vue';
const invoke=vi.hoisted(()=>vi.fn());vi.mock('@tauri-apps/api/core',()=>({invoke}));
beforeEach(()=>invoke.mockReset());
it('discovers optional receivers and sends only a live resource with explicit general scope',async()=>{
 invoke.mockResolvedValueOnce([{pluginId:'consumer',name:'Consumer'}]).mockResolvedValue({requested:true});
 const view=mount(MaterialPlannerButton,{props:{resourceId:'live-resource',games:[{id:'first-game',name:'First',status:'installed'}] as never}});await flushPromises();
 expect(view.get<HTMLSelectElement>('[data-field=material-game]').element.value).toBe('');await view.get('button').trigger('click');await flushPromises();
 expect(invoke).toHaveBeenLastCalledWith('send_blueprint_materials',{resourceId:'live-resource',targetPluginId:'consumer',gameId:null});
});
it('keeps absent consumers optional and preserves an explicitly selected game',async()=>{
 invoke.mockResolvedValueOnce([]);const absent=mount(MaterialPlannerButton,{props:{resourceId:'live',games:[]}});await flushPromises();expect(absent.find('button').exists()).toBe(false);
 invoke.mockResolvedValueOnce([{pluginId:'consumer',name:'Consumer'}]);const bound=mount(MaterialPlannerButton,{props:{resourceId:'live',games:[{id:'chosen',name:'Chosen',status:'installed'}] as never,initialGameId:'chosen'}});await flushPromises();expect(bound.get<HTMLSelectElement>('select').element.value).toBe('chosen');
});
