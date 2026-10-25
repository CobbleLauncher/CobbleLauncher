// SPDX-License-Identifier: MIT
const interactive = 'button,a,input,textarea,select,label,summary,[contenteditable],[role="button"],[role="menu"],[role="menuitem"],[role="option"],[role="slider"],[role="tab"],[tabindex],[draggable="true"],canvas,iframe,[data-no-context]';
export function editableElement(target: EventTarget | null): HTMLInputElement | HTMLTextAreaElement | HTMLElement | null {
  if (!(target instanceof Element)) return null;
  return target.closest<HTMLInputElement | HTMLTextAreaElement | HTMLElement>('input:not([disabled]),textarea:not([disabled]),[contenteditable=""],[contenteditable="true"]');
}
export function selectableElement(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement) || target.closest('button,a,[role="menu"],[role="option"]')) return null;
  return getComputedStyle(target).userSelect === 'text' || !!target.closest('pre,code,output,[data-selectable],.selectable') ? target : null;
}
export function blankWorkspaceTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && !!target.closest('.app-workspace') && target.matches('main,section,article,div,ul,ol') && !target.closest(interactive) && !selectableElement(target);
}
export function interfaceZoom(): number {
  const zoom = Number.parseFloat(getComputedStyle(document.documentElement).zoom);
  return Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
}
export function clampCenter(x: number, y: number, width: number, height: number, diameter: number) {
  const half = diameter / 2 + 8;
  return { x: Math.max(half, Math.min(width - half, x)), y: Math.max(half, Math.min(height - half, y)) };
}
