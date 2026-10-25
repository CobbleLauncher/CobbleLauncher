// SPDX-License-Identifier: MIT
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, text = '', cls = '') {
    const node = document.createElement(tag);
    node.textContent = text;
    node.className = cls;
    return node;
}
export function button(text: string, action: string, run: () => void, cls = '') {
    const node = el('button', text, cls);
    node.type = 'button'; node.dataset.action = action; node.onclick = run;
    return node;
}
export function option(node: HTMLSelectElement, value: string, text: string) {
    const item = el('option', text); item.value = value; node.append(item);
}
function field(label: string, node: HTMLInputElement | HTMLSelectElement, id: string) {
    node.dataset.field = id;
    const wrap = el('label', label); wrap.append(node); return wrap;
}
export function createView(host: HTMLElement, tr: (en: string, zh: string) => string) {
    const header = el('header'); header.append(el('h1', tr('Material Planner', '材料筹备清单')));
    const scopes = el('div', '', 'scope-row');
    const game = el('select'), world = el('select'), plans = el('select');
    scopes.append(field(tr('Game', '游戏'), game, 'game'), field(tr('World', '世界'), world, 'world'), field(tr('Plan', '计划'), plans, 'plan'));
    const error = el('div', '', 'error'); error.setAttribute('role', 'alert'); error.hidden = true;
    const errorText = el('span'); error.append(errorText);
    const reload = button(tr('Reload', '重新加载'), 'reload', () => {}); error.append(reload);
    const confirm = el('div', '', 'confirm-bar'); confirm.hidden = true;
    const confirmText = el('span');
    const accept = button(tr('Discard edits', '放弃修改'), 'confirm', () => {});
    const cancel = button(tr('Keep editing', '继续编辑'), 'cancel-confirm', () => {});
    confirm.append(confirmText, accept, cancel);
    const editor = el('section', '', 'plan-editor');
    const title = el('div', '', 'title-row'), name = el('input'); name.maxLength = 80;
    const save = button(tr('Save progress', '保存进度'), 'save', () => {}, 'primary');
    const remove = button(tr('Delete', '删除'), 'delete', () => {});
    const copy = button(tr('Copy list', '复制清单'), 'copy', () => {});
    const download = button(tr('Export text', '导出文本'), 'export', () => {});
    title.append(field(tr('Plan name', '计划名称'), name, 'name'), save, copy, download, remove);
    const source = el('p', '', 'source');
    const summary = el('p', '', 'summary'); summary.setAttribute('aria-live', 'polite');
    const tools = el('div', '', 'list-tools'), search = el('input'), sort = el('select'), stack = el('input');
    search.type = 'search'; search.placeholder = tr('Search materials', '搜索材料');
    search.setAttribute('aria-label', search.placeholder); search.dataset.field = 'search';
    for (const [value, label] of [['missing', tr('Missing: most first', '缺口从多到少')], ['required', tr('Required: most first', '需求从多到少')], ['name', tr('Material name', '材料名称')], ['collected', tr('Collected: most first', '已收集从多到少')]]) option(sort, value!, label!);
    stack.type = 'number'; stack.min = '1'; stack.max = '999'; stack.value = '64';
    const apply = button(tr('Set unknown sizes', '设置未知组数'), 'apply-stack', () => {});
    tools.append(search, field(tr('Sort', '排序'), sort, 'sort'), field(tr('Items per stack', '每组数量'), stack, 'default-stack'), apply);
    const table = el('table'), head = el('thead'), row = el('tr');
    for (const label of [tr('Material', '材料'), tr('Required', '需求'), tr('Collected', '已收集'), tr('Missing', '缺口'), tr('Stack size', '每组数量'), tr('Stacks + loose', '组 + 散件')]) row.append(el('th', label));
    head.append(row); const body = el('tbody'); table.append(head, body);
    const list = el('div', '', 'table-wrap'); list.append(table);
    const noMatch = el('p', tr('No matching materials.', '没有匹配的材料。'), 'empty'); noMatch.hidden = true;
    const note = el('p', tr('Block requirements; enter collected quantities yourself.', '统计为方块需求；已收集数量由你填写。'), 'note');
    editor.append(title, source, summary, tools, list, noMatch, note);
    const empty = el('p', tr('Send a blueprint’s material counts to start a plan.', '从蓝图发送材料统计，开始筹备计划。'), 'empty');
    const status = el('p', '', 'status'); status.setAttribute('role', 'status');
    const manualCopy = el('textarea'); manualCopy.readOnly = true; manualCopy.hidden = true; manualCopy.setAttribute('aria-label', tr('Select and copy text', '选择并复制文本'));
    host.replaceChildren(header, scopes, error, confirm, empty, editor, status, manualCopy);
    return { game, world, plans, error, errorText, reload, confirm, confirmText, accept, cancel, editor, name, save, remove, copy, download, source, summary, search, sort, stack, apply, body, noMatch, empty, status, manualCopy };
}
