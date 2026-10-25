// SPDX-License-Identifier: MIT
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, text = '', cls = '') {
    const node = document.createElement(tag); node.textContent = text; node.className = cls; return node;
}
export function button(text: string, action: string, run: () => void) {
    const node = el('button', text); node.type = 'button'; node.dataset.action = action; node.onclick = run; return node;
}
export function option(select: HTMLSelectElement, value: string, text: string) {
    const item = el('option', text); item.value = value; select.append(item);
}
function field(label: string, node: HTMLSelectElement, name: string) {
    const wrap = el('label', label); node.dataset.field = name; wrap.append(node); return wrap;
}
export function createView(host: HTMLElement, tr: (en: string, zh: string) => string) {
    const header = el('header'), pick = button(tr('Choose files', '选择文件'), 'pick', () => {}), download = button(tr('Export report', '导出报告'), 'export', () => {});
    const actions = el('div', '', 'actions'); actions.append(pick, download); header.append(el('h1', tr('Language Doctor', '语言检查')), actions);
    const error = el('p', '', 'error'); error.hidden = true; error.setAttribute('role', 'alert');
    const source = el('p', '', 'source');
    const setup = el('div', '', 'selection-row'), namespace = el('select'), baseline = el('select'), target = el('select');
    const check = button(tr('Check translations', '检查翻译'), 'check', () => {}), cancel = button(tr('Cancel', '取消'), 'cancel', () => {}); cancel.hidden = true;
    setup.append(field(tr('Namespace', '命名空间'), namespace, 'namespace'), field(tr('Reference', '基准语言'), baseline, 'baseline'), field(tr('Translation', '待检查语言'), target, 'target'), check, cancel);
    const metrics = el('div', '', 'metrics');
    const referenceCount = el('strong', '—'), targetCount = el('strong', '—'), issueCount = el('strong', '—'), format = el('strong', '—');
    for (const [value, label] of [[referenceCount, tr('Reference keys', '基准键数')], [targetCount, tr('Translation keys', '翻译键数')], [issueCount, tr('Issues', '问题')], [format, tr('Pack format', '资源包格式')]] as const) {
        const metric = el('div'); metric.append(value, el('span', label)); metrics.append(metric);
    }
    const workspace = el('div', '', 'language-layout'), inventory = el('section', '', 'issue-list'), toolbar = el('div', '', 'list-toolbar'), search = el('input'), filter = el('select');
    search.type = 'search'; search.placeholder = tr('Search keys and translations', '搜索键与翻译'); search.dataset.field = 'search'; search.setAttribute('aria-label', search.placeholder);
    filter.dataset.field = 'filter'; filter.setAttribute('aria-label', tr('Issue type', '问题类型')); toolbar.append(search, filter);
    const table = el('table'), head = el('thead'), row = el('tr');
    for (const title of [tr('Key', '键'), tr('Issue', '问题'), tr('Line', '行')]) row.append(el('th', title)); head.append(row);
    const body = el('tbody'); table.append(head, body); const tableWrap = el('div', '', 'table-wrap'); tableWrap.append(table);
    const empty = el('p', tr('Choose a resource pack ZIP or language JSON files.', '选择资源包 ZIP 或语言 JSON 文件。'), 'empty');
    inventory.append(toolbar, tableWrap, empty);
    const detail = el('section', '', 'issue-detail');
    const detailTitle = el('h2', tr('Select an issue', '选择问题')), message = el('p'), values = el('dl');
    const reference = el('dd'), translation = el('dd'); values.append(el('dt', tr('Reference', '基准文本')), reference, el('dt', tr('Translation', '翻译文本')), translation);
    const fileName = el('p', '', 'file-name'), code = el('pre'); code.setAttribute('aria-label', tr('Source lines', '原文行')); detail.append(detailTitle, message, values, fileName, code);
    workspace.append(inventory, detail);
    const notice = el('p', '', 'notice'); notice.hidden = true;
    const status = el('p', '', 'status'); status.setAttribute('role', 'status');
    host.replaceChildren(header, error, source, setup, metrics, notice, workspace, status);
    return { pick, download, error, source, namespace, baseline, target, check, cancel, metrics, referenceCount, targetCount, issueCount, format, search, filter, body, empty, detailTitle, message, values, reference, translation, fileName, code, notice, status };
}
