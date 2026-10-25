// SPDX-License-Identifier: MIT

export function navigateListWithArrows(event: KeyboardEvent, focusSelector: string) {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.altKey || event.ctrlKey || event.metaKey) return;
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
  const target = event.target;
  if (!(target instanceof HTMLElement) || target.closest('input:not([type="checkbox"]):not([type="radio"]), textarea, select, [contenteditable="true"], [role="textbox"]')) return;
  const list = event.currentTarget;
  if (!(list instanceof HTMLElement)) return;
  const row = target.closest('li');
  if (!row || row.parentElement !== list) return;
  const rows = Array.from(list.children).filter((child): child is HTMLElement => child instanceof HTMLElement && child.tagName === 'LI');
  const next = rows[rows.indexOf(row) + (event.key === 'ArrowDown' ? 1 : -1)];
  const control = next?.querySelector<HTMLElement>(focusSelector);
  if (!control) return;
  event.preventDefault();
  control.focus();
}

export function isContextMenuKey(event: KeyboardEvent) {
  return !event.defaultPrevented && !event.isComposing && event.keyCode !== 229 && !event.altKey && !event.ctrlKey && !event.metaKey
    && (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10'));
}
