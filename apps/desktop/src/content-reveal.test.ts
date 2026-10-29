// SPDX-License-Identifier: MIT
/* eslint-disable vue/one-component-per-file -- Two independent composable lifecycle fixtures. */
import { defineComponent, h, ref } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { useContentReveal, type ContentReveal } from './content-reveal';
afterEach(() => { vi.useRealTimers(); document.body.innerHTML = ''; });
it('waits for matching content, locates disabled filenames and does not replay after refresh', async () => {
  vi.useFakeTimers();
  const target = ref<ContentReveal>({ request: 1, files: ['mod.jar'] });
  const rows = ref<{file_name: string}[]>([]), prepare = vi.fn(async () => {}), scroll = vi.fn();
  const wrapper = mount(defineComponent({ setup() {
    const root = ref<HTMLElement>(); const highlighted = useContentReveal(() => target.value, rows, root, prepare);
    return () => h('div', {ref: root}, rows.value.map(row => h('div', { 'data-content-file': row.file_name, tabindex: -1, class: highlighted.value.includes(row.file_name) ? 'recent' : '', ref: (element) => { if (element) (element as HTMLElement).scrollIntoView = scroll; } }, row.file_name)));
  }}), { attachTo: document.body });
  await flushPromises(); expect(prepare).not.toHaveBeenCalled();
  rows.value = [{file_name: 'mod.jar.disabled'}]; await flushPromises();
  expect(prepare).toHaveBeenCalledWith('mod.jar.disabled'); expect(wrapper.get('.recent').element).toBe(document.activeElement); expect(scroll).toHaveBeenCalledOnce();
  rows.value = [...rows.value]; await flushPromises(); expect(prepare).toHaveBeenCalledOnce();
  await vi.advanceTimersByTimeAsync(3500); expect(wrapper.find('.recent').exists()).toBe(false); wrapper.unmount();
});
it('ignores stale preparation when a newer request has no matching rows yet', async () => {
  const target = ref<ContentReveal>({request: 1, files: ['old.jar']});
  const rows = ref([{file_name: 'old.jar'}]); let finish!: () => void;
  const wrapper = mount(defineComponent({setup() {
    const highlighted = useContentReveal(() => target.value, rows, ref(), () => new Promise<void>(resolve => { finish = resolve; }));
    return () => h('div', highlighted.value.join(','));
  }}));
  target.value = {request: 2, files: ['new.jar']}; await flushPromises(); finish(); await flushPromises();
  expect(wrapper.text()).toBe(''); wrapper.unmount();
});
