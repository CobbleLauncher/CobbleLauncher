// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext, connect, type Client } from '../../../../packages/plugin-web-sdk/src/index';
import type { MovePreview, MoveTask, StorageOverview } from '../../../../packages/plugin-web-sdk/src/storage-workspace';
import { bytes, percent, problem } from './model';
import { button, createView, el } from './view';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
export function mountStorageLens(client: Client, host = document.getElementById('app')!) {
    let zh = false, locale = 'en', busy = false, disposed = false;
    let view: ReturnType<typeof createView>, preview: MovePreview | null = null, chosen = '';
    let active: MoveTask | null = null, timer: ReturnType<typeof setTimeout> | null = null;
    let overview: StorageOverview | null = null;
    const tr = (en: string, cn: string) => zh ? cn : en;
    function fail(cause: unknown) { view.error.textContent = problem(cause, zh); view.error.hidden = false; }
    function controls() {
        host.setAttribute('aria-busy', String(busy));
        view.refresh.disabled = busy || active?.status === 'running'; view.clear.disabled = view.refresh.disabled;
        view.body.querySelectorAll('button').forEach(node => node.disabled = busy || active?.status === 'running' || node.dataset.movable === 'false');
        view.retained.querySelectorAll('button').forEach(node => node.disabled = busy || active?.status === 'running');
        view.start.disabled = busy || preview?.enoughSpace === false;
        view.dismiss.disabled = busy;
    }
    async function run(action: () => Promise<void>) {
        if (busy) return;
        busy = true; view.error.hidden = true; view.status.textContent = ''; controls();
        try { await action(); } catch (cause) { fail(cause); }
        finally { busy = false; controls(); }
    }
    async function refresh() {
        overview = await client.storageWorkspace.overview();
        view.cacheBytes.textContent = `${bytes(overview.cache.reclaimableBytes, locale)} ${tr('reclaimable', '可回收')}`;
        view.empty.hidden = overview.games.length !== 0;
        view.body.replaceChildren();
        for (const game of overview.games) {
            const row = el('tr'); row.dataset.gameId = game.gameId;
            const move = button(tr('Choose destination', '选择迁移位置'), 'pick-destination', () => void run(async () => {
                preview = await client.storageWorkspace.pickDestination({ gameId: game.gameId });
                if (!preview) return;
                chosen = game.name; view.previewTitle.textContent = `${tr('Move', '迁移')} · ${game.name}`;
                view.target.textContent = `${tr('Destination', '目标')} · ${preview.targetLabel}`;
                view.space.textContent = `${tr('Required', '需要')} ${bytes(preview.requiredBytes, locale)} · ${tr('Available', '可用')} ${preview.availableBytes === null ? tr('Unknown', '未知') : bytes(preview.availableBytes, locale)}`;
                view.preview.hidden = false;
            }));
            move.dataset.movable = String(game.movable); move.disabled = !game.movable;
            const action = el('td'); action.append(move);
            const title = el('td', game.name);
            if (game.activity && game.activity !== 'idle') title.append(el('span', game.activity === 'running' ? tr('Running', '运行中') : tr('Busy', '处理中'), 'activity'));
            row.append(title, el('td', bytes(game.instanceBytes, locale)), el('td', bytes(game.backupBytes, locale)), action);
            view.body.append(row);
        }
        view.retained.replaceChildren();
        if (overview.retained.length) {
            view.retained.append(el('h2', tr('Original copies', '旧副本')));
            view.retained.append(el('p', tr('Recycling keeps recovery available. Empty the system Recycle Bin to release disk space.', '移入回收站后仍可恢复；清空系统回收站才会释放磁盘空间。'), 'muted'));
        }
        for (const receipt of overview.retained) {
            const row = el('div', '', 'retained-row');
            row.append(el('span', `${overview.games.find(game => game.gameId === receipt.gameId)?.name ?? receipt.sourceLabel} · ${bytes(receipt.bytes, locale)}`));
            const open = button(tr('Open folder', '打开目录'), 'open-source', () => void run(async () => { await client.storageWorkspace.openSource({ taskId: receipt.taskId }); }));
            row.append(open);
            if (receipt.canRecycle) row.append(button(tr('Move to Recycle Bin', '移入回收站'), 'release-source', () => void run(async () => {
                const result = await client.storageWorkspace.releaseSource({ taskId: receipt.taskId });
                if (result.released) { await refresh(); view.status.textContent = tr('Original copy moved to Recycle Bin.', '旧副本已移入回收站。'); }
            })));
            view.retained.append(row);
        }
    }
    function drawTask() {
        if (!active) return;
        view.task.hidden = false; view.taskTitle.textContent = chosen;
        view.progress.value = percent(active.completedBytes, active.totalBytes);
        view.progress.hidden = active.status !== 'running'; view.cancel.hidden = active.status !== 'running'; view.cancel.disabled = active.phase === 'publishing';
        view.taskText.textContent = active.status === 'running' ? `${active.phase === 'publishing' ? tr('Saving location…', '正在保存新位置…') : tr('Copying and verifying…', '正在复制与核验…')} ${bytes(active.completedBytes, locale)} / ${bytes(active.totalBytes, locale)}`
            : active.status === 'succeeded' ? tr('Game moved. The original copy is kept.', '迁移完成，旧副本仍保留。') : problem(active.error ?? 'STORAGE_CANCELLED', zh);
    }
    async function poll() {
        if (!active || disposed) return;
        try {
            active = await client.storageWorkspace.task({ taskId: active.taskId }); drawTask(); controls();
            if (active.status === 'running') timer = setTimeout(() => void poll(), 700);
            else { await client.view.setDirty({ dirty: false }); await refresh(); controls(); }
        } catch (cause) { fail(cause); }
    }
    async function start() {
        const context = await hostContext(client.context()); locale = context.locale; zh = locale === 'zh-CN';
        document.documentElement.lang = locale; document.documentElement.dataset.theme = context.theme;
        host.className = 'workbench storage-workspace'; view = createView(host, tr);
        view.refresh.onclick = () => void run(refresh);
        view.clear.onclick = () => void run(async () => {
            const result = await client.storageWorkspace.cleanCache();
            if (result.cleared) { await refresh(); view.status.textContent = `${tr('Reclaimed', '已回收')} ${bytes(result.reclaimedBytes, locale)}`; }
        });
        view.dismiss.onclick = () => { preview = null; view.preview.hidden = true; };
        view.start.onclick = () => void run(async () => {
            if (!preview) return;
            const result = await client.storageWorkspace.startMove({ planId: preview.planId });
            if (!result.taskId) return;
            await client.view.setDirty({ dirty: true });
            view.preview.hidden = true; preview = null;
            active = await client.storageWorkspace.task({ taskId: result.taskId }); drawTask(); void poll();
        });
        view.cancel.onclick = () => void run(async () => { if (active) await client.storageWorkspace.cancel({ taskId: active.taskId }); });
        await run(refresh);
    }
    void start().catch(cause => { if (view) fail(cause); else host.textContent = 'Could not open Storage / 无法打开磁盘整理'; });
    return { dispose() { disposed = true; if (timer) clearTimeout(timer); } };
}
try { mountStorageLens(connect()); }
catch { document.getElementById('app')!.textContent = 'Open Storage from Cobble Launcher / 请从 Cobble Launcher 打开磁盘整理'; }

installPluginPresentation(presentation);
