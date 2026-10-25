// SPDX-License-Identifier: MIT
import { isTauri } from '@tauri-apps/api/core';
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { CommandAction } from '../commands/registry';

export interface EditLabels { copy: string; cut: string; paste: string; selectAll: string }
async function copy(text: string) { if (isTauri()) await writeText(text); else await navigator.clipboard.writeText(text); }
async function paste() { return isTauri() ? await readText() : await navigator.clipboard.readText(); }

/** Capture selection on invocation; clipboard reads occur only after an explicit Paste. */
export function editingActions(element: HTMLElement, labels: EditLabels, editable: boolean): CommandAction[] {
  const field = element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement ? element : null;
  const original = field?.value ?? element.textContent ?? '';
  const start = field?.selectionStart ?? 0;
  const end = field?.selectionEnd ?? original.length;
  const selection = window.getSelection();
  const range = !field && selection?.rangeCount && element.contains(selection.anchorNode) ? selection.getRangeAt(0).cloneRange() : null;
  const selected = field ? original.slice(start, end) : range?.toString() ?? '';
  const secret = field instanceof HTMLInputElement && field.type === 'password';
  const writable = editable && (!field || (!field.readOnly && !field.disabled));
  function restoreFocus() {
    if (!element.isConnected) return false;
    element.focus({ preventScroll: true });
    return true;
  }
  function replace(text: string) {
    if (!restoreFocus() || (field ? field.value : element.textContent ?? '') !== original) return;
    if (field) {
      const limit = field.maxLength;
      const value = limit < 0 ? text : text.slice(0, Math.max(0, limit - original.length + end - start));
      if (field.selectionStart !== null) field.setRangeText(value, start, end, 'end');
      else field.value = value;
    } else {
      const current = window.getSelection();
      const insertion = range ?? document.createRange();
      if (!range) { insertion.selectNodeContents(element); insertion.collapse(false); }
      insertion.deleteContents();
      const node = document.createTextNode(text); insertion.insertNode(node); insertion.setStartAfter(node); insertion.collapse(true);
      current?.removeAllRanges(); current?.addRange(insertion);
    }
    element.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }));
  }
  return [
    ...(!secret ? [{ id: 'edit.copy', label: labels.copy, icon: 'copy' as const, enabled: !!(selected || (!editable && original)), run: () => copy(selected || original) }] : []),
    ...(writable && !secret ? [{ id: 'edit.cut', label: labels.cut, icon: 'x' as const, enabled: !!selected, run: async () => { await copy(selected); replace(''); } }] : []),
    ...(writable ? [{ id: 'edit.paste', label: labels.paste, icon: 'copy' as const, run: async () => { const value = await paste(); if (value !== null) replace(value); } }] : []),
    { id: 'edit.selectAll', label: labels.selectAll, run: () => {
      if (!restoreFocus()) return;
      if (field) field.select();
      else { const all = document.createRange(); all.selectNodeContents(element); const current = window.getSelection(); current?.removeAllRanges(); current?.addRange(all); }
    } },
  ];
}
