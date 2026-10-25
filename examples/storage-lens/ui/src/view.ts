// SPDX-License-Identifier: MIT
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, text = '', cls = '') {
    const node = document.createElement(tag); node.textContent = text; node.className = cls; return node;
}
export function button(text: string, action: string, run: () => void) {
    const node = el('button', text); node.type = 'button'; node.dataset.action = action; node.onclick = run; return node;
}
export function createView(host: HTMLElement, tr: (en: string, zh: string) => string) {
    const header = el('header'), refresh = button(tr('Refresh', '刷新'), 'refresh', () => {});
    header.append(el('h1', tr('Storage', '磁盘整理')), refresh);
    const error = el('p', '', 'error'); error.hidden = true; error.setAttribute('role', 'alert');
    const cache = el('section', '', 'cache-row'), cacheLabel = el('div'), cacheBytes = el('strong'), clear = button(tr('Clear cache', '清理缓存'), 'clear-cache', () => {});
    cacheLabel.append(el('h2', tr('Public metadata cache', '公开资料缓存')), el('p', tr('Can be downloaded again.', '可重新下载。'), 'muted'));
    cache.append(cacheLabel, cacheBytes, clear);
    const games = el('section'), heading = el('h2', tr('Games', '游戏'));
    const table = el('table'), head = el('thead'), row = el('tr');
    for (const title of [tr('Game', '游戏'), tr('Instance', '实例文件'), tr('Backups', '世界备份'), '']) row.append(el('th', title));
    head.append(row); const body = el('tbody'); table.append(head, body);
    const wrap = el('div', '', 'table-wrap'); wrap.append(table); games.append(heading, wrap);
    const empty = el('p', tr('No games to display.', '还没有可显示的游戏。'), 'empty'); games.append(empty);
    const preview = el('section', '', 'move-preview'); preview.hidden = true;
    const previewTitle = el('h2'), target = el('p'), space = el('p', '', 'muted');
    const start = button(tr('Move game', '迁移游戏'), 'start-move', () => {}), dismiss = button(tr('Cancel', '取消'), 'dismiss-preview', () => {});
    const actions = el('div', '', 'actions'); actions.append(start, dismiss);
    preview.append(previewTitle, target, space, el('p', tr('Copy and verify first. The original copy is kept until you remove it.', '先复制并核验。旧副本将保留，直到你主动删除。'), 'muted'), actions);
    const task = el('section', '', 'move-task'); task.hidden = true;
    const taskTitle = el('h2'), progress = el('progress'), taskText = el('p'), cancel = button(tr('Cancel move', '取消迁移'), 'cancel-move', () => {});
    progress.max = 100; task.append(taskTitle, progress, taskText, cancel);
    const retained = el('section', '', 'retained');
    const status = el('p', '', 'status'); status.setAttribute('role', 'status');
    host.replaceChildren(header, error, cache, games, preview, task, retained, status);
    return { refresh, error, cacheBytes, clear, body, empty, preview, previewTitle, target, space, start, dismiss, task, taskTitle, progress, taskText, cancel, retained, status };
}
