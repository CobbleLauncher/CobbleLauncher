// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import CommandPalette from './CommandPalette.vue';

afterEach(() => { document.body.innerHTML = ''; });

it('anchors search to the titlebar without dimming the workspace', async () => {
 const anchor=document.createElement('div');document.body.append(anchor);
 vi.spyOn(anchor,'getBoundingClientRect').mockReturnValue({top:10,left:200,width:360,height:36,bottom:46,right:560,x:200,y:10,toJSON:()=>({})});
 const wrapper=mount(CommandPalette,{props:{open:true,commands:[],anchor},attachTo:document.body});await flushPromises();
 const dialog=document.querySelector<HTMLElement>('.command-dialog-header')!;
 expect(dialog.style.top).toBe('10px');expect(dialog.style.left).toBe('200px');expect(dialog.style.width).toBe('360px');
 expect(document.querySelector('.command-overlay-header')).not.toBeNull();
 expect(dialog.querySelector('.command-search-icon')).not.toBeNull();
 (dialog.querySelector('.command-search-close') as HTMLButtonElement).click();
 expect(wrapper.emitted('close')).toEqual([[]]);wrapper.unmount();
});

it('renders a bounded first page while searching every registered action', async () => {
  const actions = Array.from({ length: 1100 }, (_, index) => ({ id: `game:${index}`, label: `Fixture game ${index}`, run: vi.fn() }));
  const wrapper = mount(CommandPalette, { props: { open: true, commands: actions }, attachTo: document.body });
  await flushPromises();
  expect(document.querySelectorAll('#command-results [role="option"]')).toHaveLength(50);
  const list = document.querySelector<HTMLElement>('#command-results')!;
  Object.defineProperty(list, 'scrollHeight', { value: 1000, configurable: true });
  Object.defineProperty(list, 'clientHeight', { value: 500, configurable: true });
  list.scrollTop = 450;
  list.dispatchEvent(new Event('scroll'));
  await flushPromises();
  expect(document.querySelectorAll('#command-results [role="option"]')).toHaveLength(100);
  const input = document.querySelector<HTMLInputElement>('.command-dialog input')!;
  input.value = 'Fixture game 1099';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  await flushPromises();
  expect(document.querySelectorAll('#command-results [role="option"]')).toHaveLength(1);
  expect(document.querySelector('#command-results [role="option"]')?.textContent).toContain('1099');
  wrapper.unmount();
});
