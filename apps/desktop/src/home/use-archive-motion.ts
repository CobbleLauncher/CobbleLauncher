// SPDX-License-Identifier: MIT
import { onBeforeUnmount, onMounted, watch, type Ref } from 'vue';

/** Shared motion for the sheets and their matching interactive surfaces. */
export function useArchiveMotion(stack: Ref<HTMLElement | undefined>, focus: Ref<number>) {
  const current = { x: 0, y: 0, focus: 0, selection: 0, engagement: 0 };
  const target = { ...current };
  let frame = 0, previousTime = 0, focusOffset = 0;
  let media: MediaQueryList | undefined;
  const reduced = () => media?.matches || stack.value?.closest('[data-motion="reduced"]');
  const clamp = (value: number) => Math.max(-1, Math.min(1, value));

  function paint() {
    const element = stack.value;
    if (!element) return;
    element.style.setProperty('--drift-x', `${(current.x * 7).toFixed(3)}px`);
    element.style.setProperty('--drift-y', `${(current.y * 5).toFixed(3)}px`);
    element.style.setProperty('--tilt-x', `${(-current.y * 1.4).toFixed(3)}deg`);
    element.style.setProperty('--tilt-y', `${(current.x * 1.8).toFixed(3)}deg`);
    element.style.setProperty('--glint-x', `${(current.x * 18).toFixed(3)}%`);
    element.querySelectorAll<HTMLElement>('.home-archive-card').forEach((card, index) => {
      // A smooth spacing field opens both sides of the preview, without popping it forward.
      const separation = Math.tanh((index - current.focus) * .85) * current.engagement;
      card.style.setProperty('--fan-offset', separation.toFixed(4));
      card.style.setProperty('--front-open', (Math.max(0, Math.min(1, current.selection - index)) * current.engagement).toFixed(4));
    });
  }
  function stop() { cancelAnimationFrame(frame); frame = 0; previousTime = 0; }
  function reset() {
    stop();
    focusOffset = 0;
    Object.assign(current, { x: 0, y: 0, focus: Math.max(0, focus.value), selection: Math.max(0, focus.value), engagement: 0 });
    Object.assign(target, current);
    paint();
  }
  function tick(time: number) {
    frame = 0;
    if (!stack.value || document.hidden || reduced()) { reset(); return; }
    const dt = previousTime ? Math.min(48, time - previousTime) : 16;
    previousTime = time;
    const blend = 1 - Math.exp(-dt / 85);
    let moving = false;
    for (const key of ['x', 'y', 'focus', 'selection', 'engagement'] as const) {
      current[key] += (target[key] - current[key]) * blend;
      if (Math.abs(target[key] - current[key]) < .001) current[key] = target[key];
      else moving = true;
    }
    paint();
    if (moving) frame = requestAnimationFrame(tick);
    else previousTime = 0;
  }
  function schedule() {
    if (document.hidden || reduced()) { reset(); return; }
    if (!frame) frame = requestAnimationFrame(tick);
  }
  function move(event: PointerEvent) {
    if (event.pointerType === 'touch' || !stack.value) return;
    const bounds = stack.value.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const scene = stack.value.querySelector<HTMLElement>('.home-archive-scene');
    if (scene) {
      const spaceBelow = bounds.bottom - scene.getBoundingClientRect().bottom - 16;
      stack.value.style.setProperty('--front-travel', `${Math.max(0, Math.min(64, spaceBelow))}px`);
    }
    target.x = clamp((event.clientX - bounds.left) / bounds.width * 2 - 1);
    target.y = clamp((event.clientY - bounds.top) / bounds.height * 2 - 1);
    // Spacing responds even while moving within the same sheet.
    focusOffset = (target.x - target.y) * .16;
    target.focus = Math.max(0, focus.value) + focusOffset;
    target.engagement = .85 + (target.x - target.y) * .075;
    schedule();
  }
  function leave() {
    focusOffset = 0; target.focus = Math.max(0, focus.value);
    target.x = 0; target.y = 0; target.engagement = 0;
    schedule();
  }
  watch(focus, value => { target.selection = Math.max(0, value); target.focus = target.selection + focusOffset; schedule(); });
  function visibility() { if (document.hidden) reset(); }
  onMounted(() => {
    current.focus = target.focus = Math.max(0, focus.value);
    current.selection = target.selection = Math.max(0, focus.value);
    media = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    media?.addEventListener('change', reset);
    document.addEventListener('visibilitychange', visibility);
  });
  onBeforeUnmount(() => {
    stop(); media?.removeEventListener('change', reset);
    document.removeEventListener('visibilitychange', visibility);
  });
  return { move, leave };
}
