// SPDX-License-Identifier: MIT
import type { Book } from './model';
import { NotesEditor, validNote } from './notes-model';
import { button, el, option } from './view';
export function createNotesView(editor: NotesEditor, tr: (en: string, zh: string) => string, actions: { changed: () => void; leave: (run: () => void) => void; save: () => void; remove: () => void; openLocation: (id: string) => void }) {
    const root = el('section', '', 'world-notes book-layout'), inventory = el('aside', '', 'inventory'), tools = el('div', '', 'list-toolbar'), search = el('input'), filter = el('select'), list = el('div', '', 'location-list');
    search.type = 'search'; search.placeholder = tr('Find notes', '查找笔记'); search.setAttribute('aria-label', search.placeholder); search.dataset.field = 'note-search';
    filter.setAttribute('aria-label', tr('Show notes', '显示笔记')); filter.dataset.field = 'note-filter';
    option(filter, 'active', tr('Next session', '下次继续')); option(filter, 'all', tr('All notes', '全部笔记')); option(filter, 'completed', tr('Completed', '已完成'));
    const add = button(tr('Add note', '添加笔记'), () => actions.leave(() => { editor.select(null); actions.changed(); }), 'note-new'); tools.append(search, filter, add); inventory.append(tools, list);
    const panel = el('section', '', 'inventory'), fields = el('fieldset'), title = el('input'), body = el('textarea'), location = el('select'), completed = el('input'), dirty = el('span', tr('Unsaved', '未保存'), 'dirty-label');
    title.maxLength = 100; body.maxLength = 2000; body.rows = 7; completed.type = 'checkbox';
    const field = (label: string, node: HTMLElement, name: string) => { const wrapper = el('label', label); node.dataset.field = name; wrapper.append(node); return wrapper; };
    const open = button(tr('Open location', '打开地点'), () => actions.openLocation(location.value), 'note-open-location');
    const save = button(tr('Save note', '保存笔记'), actions.save, 'note-save', 'primary'), discard = button(tr('Discard changes', '放弃修改'), () => { editor.select(editor.draft.id); actions.changed(); }, 'note-discard'), remove = button(tr('Delete note', '删除笔记'), actions.remove, 'note-delete', 'danger'), footer = el('div', '', 'editor-actions');
    footer.append(save, dirty, discard, remove); const completeField = field(tr('Completed', '已完成'), completed, 'note-completed'); completeField.className = 'note-complete';
    fields.append(field(tr('Title', '标题'), title, 'note-title'), field(tr('Notes', '笔记'), body, 'note-body'), field(tr('Linked location', '关联地点'), location, 'note-location'), open, completeField, footer); panel.append(fields); root.append(inventory, panel);
    for (const [node, key] of [[title, 'title'], [body, 'body'], [location, 'locationId'], [completed, 'completed']] as const) node.addEventListener(node === location || node === completed ? 'change' : 'input', () => { editor.draft = {...editor.draft, [key]: node === completed ? completed.checked : node.value}; actions.changed(); });
    let book: Book = {schemaVersion: 1, locations: []}, locked = false;
    function render(nextBook: Book, busy: boolean) {
        book = nextBook; locked = busy; fields.disabled = busy || !editor.loaded; add.disabled = fields.disabled;
        title.value = editor.draft.title; body.value = editor.draft.body; completed.checked = editor.draft.completed;
        location.replaceChildren(); option(location, '', tr('No location', '不关联地点')); for (const item of book.locations) option(location, item.id, item.name);
        if (editor.draft.locationId && !book.locations.some(n => n.id === editor.draft.locationId)) option(location, editor.draft.locationId, tr('Deleted location', '地点已删除'));
        location.value = editor.draft.locationId; open.disabled = !book.locations.some(n => n.id === location.value); dirty.hidden = !editor.dirty;
        save.disabled = fields.disabled || !editor.dirty || !validNote(editor.draft); discard.disabled = fields.disabled || !editor.dirty; remove.hidden = !editor.draft.id;
        renderList();
    }
    function renderList() {
        list.replaceChildren(); const needle = search.value.trim().toLocaleLowerCase();
        const rows = editor.document.notes.filter(n => (filter.value === 'all' || n.completed === (filter.value === 'completed')) && `${n.title} ${n.body} ${book.locations.find(l => l.id === n.locationId)?.name ?? ''}`.toLocaleLowerCase().includes(needle));
        if (!rows.length) list.append(el('p', !editor.loaded ? tr('Notes unavailable.', '笔记暂不可用。') : needle ? tr('No matching notes.', '没有匹配的笔记。') : tr('No notes here yet.', '这里还没有笔记。'), 'empty'));
        for (const note of rows) { const row = button('', () => actions.leave(() => { editor.select(note.id); actions.changed(); }), 'note-select', 'location-row'); row.dataset.active = String(note.id === editor.draft.id); row.setAttribute('aria-pressed', String(note.id === editor.draft.id)); row.append(el('strong', `${note.completed ? '✓ ' : ''}${note.title}`)); const place = book.locations.find(l => l.id === note.locationId); if (place) row.append(el('span', place.name)); row.disabled = locked; list.append(row); }
    }
    search.oninput = renderList; filter.onchange = renderList;
    return {root, render};
}
