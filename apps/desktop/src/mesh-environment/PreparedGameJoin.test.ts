// SPDX-License-Identifier: MIT
import {mount} from '@vue/test-utils';
import {afterEach,beforeEach,it,expect,vi} from 'vitest';
import {setLocale} from '../i18n';
import type {Game} from '../games';
import PreparedGameJoin from './PreparedGameJoin.vue';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const game=(changes:Partial<Game>={}):Game=>({id:'new-independent-game',name:'Prepared Adventure',version:'1.21.1',directory:'',status:'installed',error:null,progress:null,content_installing:false,...changes});
beforeEach(()=>setLocale('en-US'));afterEach(()=>setLocale('zh-CN'));
it('waits for the actual new game to finish all installation work',async()=>{const wrapper=mount(PreparedGameJoin,{props:{game:null,invitation:'original-invitation'}});expect(wrapper.find('button').exists()).toBe(false);await wrapper.setProps({game:game({status:'installing'})});expect(wrapper.find('button').exists()).toBe(false);await wrapper.setProps({game:game({content_installing:true})});expect(wrapper.find('button').exists()).toBe(false);await wrapper.setProps({game:game({error:'configuration conflict'})});expect(wrapper.find('button').exists()).toBe(false);await wrapper.setProps({game:game()});expect(wrapper.get('button').text()).toBe('Join with prepared game');wrapper.unmount();});
it('explicitly carries the prepared ID and retained invite into the existing join flow',async()=>{const wrapper=mount(PreparedGameJoin,{props:{game:game(),invitation:'retained-private-invitation'}});expect(wrapper.emitted('join')).toBeUndefined();await wrapper.get('button').trigger('click');expect(wrapper.emitted('join')).toEqual([[{gameId:'new-independent-game',invitation:'retained-private-invitation'}]]);wrapper.unmount();});
it('keeps the action disabled under the parent guard and hides it without an invitation',async()=>{setLocale('zh-CN');const wrapper=mount(PreparedGameJoin,{props:{game:game(),invitation:'invite',disabled:true}});expect(wrapper.get('button').text()).toBe('使用准备好的游戏加入');await wrapper.get('button').trigger('click');expect(wrapper.emitted('join')).toBeUndefined();await wrapper.setProps({invitation:''});expect(wrapper.find('button').exists()).toBe(false);wrapper.unmount();});
