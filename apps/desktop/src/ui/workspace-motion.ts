// SPDX-License-Identifier: MIT
export function createWorkspaceMotion() {
  let current: Animation | undefined;
  function cancel() { current?.cancel(); current = undefined; }
  function play(element: HTMLElement | null, direction: 'forward' | 'back' | 'peer') {
    cancel();
    if (!element || typeof element.animate !== 'function' || window.matchMedia('(prefers-reduced-motion: reduce)').matches || element.closest('[data-motion="reduced"]')) return;
    const transform = direction === 'peer' ? 'translateY(4px)' : `translateX(${direction === 'back' ? -8 : 8}px)`;
    current = element.animate([{ opacity: .65, transform }, { opacity: 1, transform: 'none' }], { duration: 160, easing: 'cubic-bezier(.2,.7,.2,1)' });
  }
  return { play, cancel };
}
