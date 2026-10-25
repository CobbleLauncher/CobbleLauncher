// SPDX-License-Identifier: MIT
export const el = <K extends keyof HTMLElementTagNameMap>(tag: K, text = '', className = '') => {
    const node = document.createElement(tag);
    node.textContent = text;
    node.className = className;
    return node;
};
export const button = (label: string, run: () => void, action: string, cls = '') => {
    const node = el('button', label, cls);
    node.type = 'button';
    node.dataset.action = action;
    node.onclick = run;
    return node;
};
export function option(select: HTMLSelectElement, value: string, label: string) {
    const node = el('option', label);
    node.value = value;
    select.append(node);
}
function field(label: string, node: HTMLElement, name: string) {
    node.dataset.field = name;
    const wrapper = el('label', label);
    wrapper.append(node);
    return wrapper;
}
export function createView(host: HTMLElement, tr: (en: string, zh: string) => string, actions: {
    showBook: () => void;
    showPlanner: () => void;
    showNotes: () => void;
    new: () => void;
    save: () => void;
    discard: () => void;
    remove: () => void;
    copy: () => void;
    reload: () => void;
    convert: () => void;
    confirm: () => void;
    cancel: () => void;
}) {
    const header = el('header', '', 'toolbar'), title = el('h1', tr('Coordinate Book', '坐标簿')), status = el('p', '', 'status');
    status.role = 'status';
    status.setAttribute('aria-live', 'polite');
    header.append(title, status);
    const scope = el('div', '', 'book-scope'), game = el('select'), gameText = el('span', '', 'context-value'), world = el('select'), gameField = field(tr('Game', '游戏'), game, 'game'), worldField = field(tr('World or server', '世界或服务器'), world, 'world');
    gameText.hidden = true;
    gameField.append(gameText);
    scope.append(gameField, worldField);
    const error = el('div', '', 'error'), errorText = el('span'), reload = button(tr('Reload saved data', '重新加载已保存数据'), actions.reload, 'reload');
    error.role = 'alert';
    error.hidden = true;
    error.append(errorText, reload);
    const tabs = el('nav', '', 'book-tabs');
    const bookTab = button(tr('Locations', '地点'), actions.showBook, 'show-book', 'tab-button');
    const plannerTab = button(tr('Portal planner', '传送门规划'), actions.showPlanner, 'show-planner', 'tab-button');
    tabs.setAttribute('aria-label', tr('Coordinate Book views', '坐标簿视图'));
    bookTab.setAttribute('aria-pressed', 'true');
    plannerTab.setAttribute('aria-pressed', 'false');
    const notesTab = button(tr('World notes', '世界笔记'), actions.showNotes, 'show-notes', 'tab-button');
    tabs.append(bookTab, plannerTab, notesTab);
    const confirmBar = el('div', '', 'confirm-bar'), confirmText = el('span'), confirm = button(tr('Discard changes', '放弃修改'), actions.confirm, 'confirm'), cancel = button(tr('Keep editing', '继续编辑'), actions.cancel, 'cancel-confirm');
    confirmBar.hidden = true;
    confirmBar.append(confirmText, confirm, cancel);
    const layout = el('div', '', 'book-layout'), inventory = el('aside', '', 'book-list inventory'), listToolbar = el('div', '', 'list-toolbar'), search = el('input'), add = button(tr('Add location', '添加地点'), actions.new, 'new');
    search.type = 'search';
    search.placeholder = tr('Find locations', '查找地点');
    search.setAttribute('aria-label', search.placeholder);
    search.dataset.field = 'search';
    listToolbar.append(search, add);
    const list = el('div', '', 'location-list');
    inventory.append(listToolbar, list);
    const panel = el('section', '', 'book-editor inventory'), form = el('form', '', 'editor-form'), fieldset = el('fieldset'), dirty = el('span', tr('Unsaved', '未保存'), 'dirty-label');
    dirty.hidden = true;
    const name = el('input'), dimension = el('select'), x = el('input'), y = el('input'), z = el('input'), note = el('textarea');
    name.maxLength = 80;
    name.required = true;
    note.maxLength = 512;
    note.rows = 3;
    name.autocomplete = 'off';
    for (const [value, label] of [['overworld', tr('Overworld', '主世界')], ['nether', tr('Nether', '下界')], ['end', tr('End', '末地')]])
        option(dimension, value!, label!);
    for (const input of [x, y, z]) {
        input.type = 'number';
        input.step = '1';
        input.min = input === y ? '-4096' : '-29999984';
        input.max = input === y ? '4096' : '29999984';
    }
    x.required = z.required = true;
    y.placeholder = tr('Optional', '可选');
    const top = el('div', '', 'editor-top'), coords = el('div', '', 'coordinate-fields');
    top.append(field(tr('Name', '名称'), name, 'name'), field(tr('Dimension', '维度'), dimension, 'dimension'));
    coords.append(field('X', x, 'x'), field('Y', y, 'y'), field('Z', z, 'z'));
    const conversion = el('div', '', 'conversion'), conversionText = el('span'), conversionButton = button('', actions.convert, 'convert');
    conversion.hidden = true;
    conversion.append(conversionText, conversionButton);
    const footer = el('div', '', 'editor-actions'), save = button(tr('Save location', '保存地点'), actions.save, 'save', 'primary'), discard = button(tr('Discard changes', '放弃修改'), actions.discard, 'discard'), copy = button(tr('Copy coordinates', '复制坐标'), actions.copy, 'copy'), remove = button(tr('Delete', '删除'), actions.remove, 'delete', 'danger');
    footer.append(save, dirty, discard, copy, remove);
    fieldset.append(top, coords, field(tr('Note', '备注'), note, 'note'), conversion, footer);
    form.append(fieldset);
    panel.append(form);
    layout.append(inventory, panel);
    host.replaceChildren(header, scope, tabs, error, confirmBar, layout);
    form.onsubmit = event => event.preventDefault();
    return { game, gameText, world, error, errorText, reload, status, tabs, bookTab, plannerTab, notesTab, layout, confirmBar, confirmText, confirm, cancel, search, add, list, fieldset, dirty, name, dimension, x, y, z, note, conversion, conversionText, conversionButton, save, discard, copy, remove };
}
