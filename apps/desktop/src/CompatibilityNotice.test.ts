// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { it, expect, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CompatibilityNotice from './CompatibilityNotice.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
it('blocks unsupported targets and shows experimental Java before enabling', async () => {
 setLocale('en-US');
 vi.mocked(invoke).mockResolvedValueOnce({status:'unsupported',java_major:null}).mockResolvedValueOnce({status:'experimental',java_major:17});
 const wrapper=mount(CompatibilityNotice,{props:{minecraftVersion:'1.20.4',loaderKind:'forge'}}); await flushPromises();
 expect(wrapper.text()).toContain('unsupported'); expect(wrapper.emitted('blocked')?.at(-1)).toEqual([true]);
 await wrapper.setProps({minecraftVersion:'1.20.1',loaderVersion:'47.4.28'}); await flushPromises();
 expect(wrapper.text()).toContain('Java 17'); expect(wrapper.emitted('blocked')?.at(-1)).toEqual([false]);
 wrapper.unmount(); setLocale('zh-CN');
});
it('ignores a stale response after selection changes and keeps vanilla quiet', async () => {
 let resolve!: (value: unknown) => void;
 vi.mocked(invoke).mockImplementationOnce(() => new Promise(done => {resolve=done;}));
 const wrapper=mount(CompatibilityNotice,{props:{minecraftVersion:'1.21.4',loaderKind:'neoforge'}});
 await wrapper.setProps({loaderKind:'vanilla'}); resolve({status:'unsupported',java_major:null}); await flushPromises();
 expect(wrapper.text()).toBe(''); expect(wrapper.emitted('blocked')?.at(-1)).toEqual([false]); wrapper.unmount();
});
