// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { editingActions } from './edit-actions';
import { workspaceActions } from './workspace-actions';
const { readText, writeText } = vi.hoisted(() => ({ readText: vi.fn(), writeText: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ readText, writeText }));
const labels = { copy: 'Copy', cut: 'Cut', paste: 'Paste', selectAll: 'Select all' };
function input(value: string, type = 'text') { const field = document.createElement('input'); field.type = type; field.value = value; document.body.append(field); field.focus(); return field; }
beforeEach(() => { vi.clearAllMocks(); readText.mockResolvedValue('new'); writeText.mockResolvedValue(undefined); });
afterEach(() => { document.body.replaceChildren(); window.getSelection()?.removeAllRanges(); });
describe('desktop editing actions', () => {
  it('reads the clipboard only on Paste and replaces the captured selection with a normal input event', async () => {
    const field = input('before-after'); field.setSelectionRange(0, 6); const changed = vi.fn(); field.addEventListener('input', changed);
    const actions = editingActions(field, labels, true);
    expect(readText).not.toHaveBeenCalled();
    await actions.find(item => item.id === 'edit.paste')!.run();
    expect(readText).toHaveBeenCalledOnce(); expect(field.value).toBe('new-after'); expect(changed).toHaveBeenCalledOnce();
  });
  it('copies selection and cuts only after a successful clipboard write', async () => {
    const field = input('abcdef'); field.setSelectionRange(1, 3);
    const actions = editingActions(field, labels, true);
    writeText.mockRejectedValueOnce(new Error('clipboard unavailable'));
    await expect(actions.find(item => item.id === 'edit.cut')!.run()).rejects.toThrow(); expect(field.value).toBe('abcdef');
    await actions.find(item => item.id === 'edit.cut')!.run(); expect(writeText).toHaveBeenLastCalledWith('bc'); expect(field.value).toBe('adef');
  });
  it('does not overwrite a field that changed while clipboard access was pending', async () => {
    const field = input('old'); field.select(); const actions = editingActions(field, labels, true);
    field.value = 'edited'; await actions.find(item => item.id === 'edit.paste')!.run(); expect(field.value).toBe('edited');
  });
  it('keeps password content out of Copy/Cut and readonly fields out of Paste', () => {
    const password = input('private', 'password'); password.select();
    expect(editingActions(password, labels, true).map(action => action.id)).toEqual(['edit.paste', 'edit.selectAll']);
    const readonly = input('path'); readonly.readOnly = true;
    expect(editingActions(readonly, labels, true).map(action => action.id)).toEqual(['edit.copy', 'edit.selectAll']);
  });
  it('copies selected output and offers select-all without changing content', async () => {
    const output = document.createElement('pre'); output.textContent = 'one two'; document.body.append(output);
    const actions = editingActions(output, labels, false); await actions.find(action => action.id === 'edit.selectAll')!.run();
    expect(window.getSelection()?.toString()).toBe('one two');
    await actions.find(action => action.id === 'edit.copy')!.run(); expect(writeText).toHaveBeenCalledWith('one two'); expect(output.textContent).toBe('one two');
  });
  it('uses the registered actions and omits unavailable/destructive commands from blank-space menus', () => {
    const run = vi.fn(); const commands = [{ id: 'game.create', label: 'Add', run }, { id: 'settings.open', label: 'Settings', enabled: false, run }, { id: 'game.delete', label: 'Delete', run }];
    expect(workspaceActions('library', commands)).toEqual([commands[0]]); expect(run).not.toHaveBeenCalled();
  });
});
