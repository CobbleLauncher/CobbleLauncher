// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import UndoNotice from './UndoNotice.vue';
it('keeps Undo available while the user is hovering or using its keyboard controls', async () => {
  vi.useFakeTimers();
  const wrapper = mount(UndoNotice, { attachTo: document.body, props: { message: 'Archived game' } });
  try {
    await wrapper.trigger('mouseenter'); await vi.advanceTimersByTimeAsync(20000); expect(wrapper.emitted('dismiss')).toBeUndefined();
    wrapper.get('button').element.focus(); await wrapper.trigger('mouseleave');
    await vi.advanceTimersByTimeAsync(20000); expect(wrapper.emitted('dismiss')).toBeUndefined();
    await wrapper.get('button').trigger('click'); expect(wrapper.emitted('undo')).toHaveLength(1);
    wrapper.get('button').element.blur(); await vi.advanceTimersByTimeAsync(12000); expect(wrapper.emitted('dismiss')).toHaveLength(1);
  } finally { wrapper.unmount(); vi.useRealTimers(); }
});
