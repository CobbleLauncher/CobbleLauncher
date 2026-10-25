// SPDX-License-Identifier: MIT
import { groups, integer, missing, sortedItems, stackSize, type Plan, type SortOrder } from './model.ts';
import { el } from './view.ts';
export function renderMaterials(body: HTMLTableSectionElement, plan: Plan, query: string, sort: SortOrder, tr: (en: string, zh: string) => string, changed: () => void): number {
    const items = sortedItems(plan, query, sort);
    const fragment = document.createDocumentFragment();
    for (const item of items) {
        const row = el('tr'); row.dataset.material = item.id;
        const label = el('td', '', 'material-name'); label.append(el('strong', item.label));
        if (item.label !== item.id) label.append(el('small', item.id));
        const required = el('td', item.required.toLocaleString());
        const collectedCell = el('td'), collected = el('input');
        collected.type = 'number'; collected.min = '0'; collected.max = '1000000000'; collected.value = String(item.collected);
        collected.dataset.field = 'collected'; collected.setAttribute('aria-label', `${tr('Collected', '已收集')} · ${item.label}`);
        const missingCell = el('td'), stackCell = el('td'), stack = el('input'), grouped = el('td');
        stack.type = 'number'; stack.min = '1'; stack.max = '999'; stack.value = item.stackSize === null ? '' : String(item.stackSize);
        stack.placeholder = '?'; stack.dataset.field = 'stack-size'; stack.setAttribute('aria-label', `${tr('Stack size', '每组数量')} · ${item.label}`);
        function update() {
            missingCell.textContent = missing(item).toLocaleString();
            const counts = groups(missing(item), item.stackSize);
            grouped.textContent = counts ? `${counts.stacks.toLocaleString()} + ${counts.loose}` : tr('Unknown', '未知');
            row.dataset.complete = String(missing(item) === 0);
        }
        collected.oninput = () => {
            const value = integer(collected.value);
            collected.setCustomValidity(value === null ? tr('Enter a whole number from 0 to 1,000,000,000.', '请输入 0 至 1,000,000,000 的整数。') : '');
            if (value !== null) { item.collected = value; update(); changed(); }
        };
        stack.oninput = () => {
            const value = stackSize(stack.value), valid = !stack.value.trim() || value !== null;
            stack.setCustomValidity(valid ? '' : tr('Enter 1–999 or leave blank.', '请输入 1–999，或留空。'));
            if (valid) { item.stackSize = value; update(); changed(); }
        };
        collectedCell.append(collected); stackCell.append(stack);
        row.append(label, required, collectedCell, missingCell, stackCell, grouped); update(); fragment.append(row);
    }
    body.replaceChildren(fragment);
    return items.length;
}
