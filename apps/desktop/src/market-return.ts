// SPDX-License-Identifier: MIT
import { nextTick, type Ref } from 'vue';

/** Keep the surrounding native scroll container and result focus on detail return. */
export function marketReturn(root: Ref<HTMLElement | undefined>) {
  let project = '';
  let positions: { element: Element; top: number }[] = [];
  function enter(id: string) {
    project = id;
    positions = [];
    for (let element = root.value?.parentElement; element; element = element.parentElement) {
      positions.push({ element, top: element.scrollTop });
    }
    const scrolling = document.scrollingElement;
    if (scrolling && !positions.some(position => position.element === scrolling)) positions.push({ element: scrolling, top: scrolling.scrollTop });
    void nextTick(() => { for (const { element } of positions) element.scrollTop = 0; });
  }
  function restore() {
    void nextTick(() => {
      const action = [...(root.value?.querySelectorAll<HTMLButtonElement>('[data-market-project]') ?? [])]
        .find(button => button.dataset.marketProject === project);
      action?.focus({ preventScroll: true });
      for (const { element, top } of positions) element.scrollTop = top;
    });
  }
  return { enter, restore };
}
