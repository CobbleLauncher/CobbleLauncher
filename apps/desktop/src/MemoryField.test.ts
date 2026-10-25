// SPDX-License-Identifier: MIT
import {defineComponent,h,ref} from 'vue';
import {mount} from '@vue/test-utils';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import MemoryField from './MemoryField.vue';
import {setLocale} from './i18n';
beforeEach(()=>{setLocale('en-US');vi.stubGlobal('ResizeObserver',class{observe(){}unobserve(){}disconnect(){}});});
afterEach(()=>{setLocale('zh-CN');vi.unstubAllGlobals();});
const props={modelValue:4096,label:'Maximum memory',id:'memory',disabled:false};
it('preserves exact numeric MiB values and the existing allocation bounds',async()=>{
 const wrapper=mount(MemoryField,{props});const input=wrapper.get('input');expect(input.attributes()).toMatchObject({min:'512',max:'262144',step:'1','aria-label':'Maximum memory'});
 await input.setValue('6145');expect(wrapper.emitted('update:modelValue')).toEqual([[6145]]);await wrapper.setProps({modelValue:6145});expect(wrapper.get('[role="slider"]').attributes('aria-valuetext')).toBe('6145 MiB');wrapper.unmount();
});
it('supports slider keyboard allocation changes and both allowed endpoints',async()=>{
 const Harness=defineComponent({setup(){const memory=ref(4096);return()=>h(MemoryField,{...props,modelValue:memory.value,'onUpdate:modelValue':(value:number)=>{memory.value=value;}});}});
 const wrapper=mount(Harness);const thumb=wrapper.get('[role="slider"]');await thumb.trigger('keydown',{key:'ArrowRight'});expect(wrapper.get<HTMLInputElement>('input').element.value).toBe('6144');
 await thumb.trigger('keydown',{key:'Home'});expect(wrapper.get<HTMLInputElement>('input').element.value).toBe('512');await thumb.trigger('keydown',{key:'End'});expect(wrapper.get<HTMLInputElement>('input').element.value).toBe('262144');
 await wrapper.get('input').setValue('6145');expect(wrapper.get('[role="slider"]').attributes('aria-valuetext')).toBe('6145 MiB');wrapper.unmount();
});
it('keeps disabled inputs and slider gestures from editing the draft',async()=>{
 const wrapper=mount(MemoryField,{props:{...props,disabled:true}});expect(wrapper.get('input').attributes('disabled')).toBeDefined();expect(wrapper.get('.numeric-slider').attributes('aria-disabled')).toBe('true');
 await wrapper.get('[role="slider"]').trigger('keydown',{key:'ArrowRight'});await wrapper.get('input').setValue('8192');expect(wrapper.emitted('update:modelValue')).toBeUndefined();wrapper.unmount();
});
