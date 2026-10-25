// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext, connect, MATERIALS_IMPORT_INTERFACE, MATERIALS_IMPORT_VERSION, type Client, type DataScope, type Context } from '../../../../packages/plugin-web-sdk/src/index';
import { exportName, importedPlan, planText, stackSize, totals, type Plan, type SortOrder } from './model.ts';
import { PlanRepository, newId } from './storage.ts';
import { createView, option } from './view.ts';
import { renderMaterials } from './table.ts';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
export function mountMaterialPlanner(client: Client, host = document.getElementById('app')!) {
    let zh = false, busy = false, dirty = false, draft: Plan | null = null;
    let context: Context, view: ReturnType<typeof createView>;
    let pending: (() => Promise<void>) | null = null;
    let guardTail: Promise<unknown> = Promise.resolve();
    const repository = new PlanRepository(client.data);
    const tr = (en: string, cn: string) => zh ? cn : en;
    const scope = (): DataScope => ({ ...(view.game.value ? { gameId: view.game.value } : {}), ...(view.game.value && view.world.value ? { worldId: view.world.value } : {}) });
    function fail(cause: unknown) {
        const code = String(cause);
        view.error.hidden = false;
        view.errorText.textContent = /CONFLICT|RELOAD_REQUIRED/.test(code) ? tr('Saved data changed. Reload, then retry your draft.', '已保存的数据发生变化，请重新加载后重试草稿。')
            : /QUOTA|TOO_LARGE|LIMIT/.test(code) ? tr('Storage is full. Your draft is kept; export it or remove another plan.', '存储已满。草稿已保留，可导出或删除其他计划。')
            : /INCOMPLETE/.test(code) ? tr('This plan could not be read completely. Reload or delete its list entry.', '无法完整读取此计划，请重新加载或删除列表项。')
            : /INDEX_PENDING/.test(code) ? tr('Progress saved; the plan list needs a retry. Your draft is kept.', '进度已保存，计划列表更新失败。草稿已保留，请重试。')
            : /PERMISSION|REVOKED/.test(code) ? tr('Allow this tool’s storage permissions to continue.', '请允许此工具的存储权限后继续。')
            : tr('Could not load or save the plan. Your draft is unchanged.', '无法加载或保存计划，草稿未更改。');
    }
    function protect() {
        const next = dirty;
        guardTail = guardTail.catch(() => {}).then(() => client.view.setDirty({ dirty: next }));
        void guardTail.catch(() => fail(Error('DIRTY_GUARD_FAILED')));
        return guardTail;
    }
    function summary() {
        if (!draft) return;
        const sum = totals(draft);
        view.summary.textContent = tr(`${sum.complete} / ${draft.items.length} materials ready · ${sum.missing.toLocaleString()} blocks missing`, `${sum.complete} / ${draft.items.length} 种材料已备齐 · 缺 ${sum.missing.toLocaleString()} 个方块`);
        view.save.disabled = busy || !dirty;
    }
    function edited() { dirty = true; summary(); void protect(); }
    function listOptions() {
        const selected = draft?.id ?? '';
        view.plans.replaceChildren();
        option(view.plans, '', dirty && !selected ? tr('Unsaved plan', '未保存的计划') : tr('Select a plan', '选择计划'));
        for (const plan of repository.index.plans) option(view.plans, plan.id, plan.name);
        view.plans.value = selected;
    }
    function rows() {
        if (draft) view.noMatch.hidden = renderMaterials(view.body, draft, view.search.value, view.sort.value as SortOrder, tr, edited) !== 0;
        view.body.querySelectorAll('input').forEach(input => input.disabled = busy);
    }
    function render() {
        host.setAttribute('aria-busy', String(busy));
        view.editor.hidden = !draft; view.empty.hidden = !!draft;
        view.remove.hidden = !draft?.id && !view.plans.value;
        view.game.disabled = busy || !!context.instanceId; view.world.disabled = busy || !view.game.value;
        view.plans.disabled = busy; view.name.disabled = busy;
        view.search.disabled = busy; view.sort.disabled = busy; view.stack.disabled = busy;
        for (const node of [view.save, view.remove, view.copy, view.download, view.apply]) node.disabled = busy;
        view.editor.querySelectorAll<HTMLInputElement>('tbody input').forEach(node => node.disabled = busy);
        if (draft) { view.name.value = draft.name; view.source.textContent = draft.source; view.source.hidden = !draft.source; summary(); }
    }
    async function run(action: () => Promise<void>) {
        if (busy) return;
        busy = true; view.status.textContent = ''; render();
        try { await action(); } catch (cause) { fail(cause); }
        finally { busy = false; render(); }
    }
    async function leave(action: () => Promise<void>) {
        if (!dirty) { await run(action); return; }
        pending = action; view.confirm.hidden = false;
        view.confirmText.textContent = tr('Discard unsaved changes?', '放弃未保存的修改？');
        view.accept.textContent = tr('Discard edits', '放弃修改');
    }
    async function worlds(selected = '') {
        view.world.replaceChildren(); option(view.world, '', tr('All worlds', '全部世界'));
        if (!view.game.value) return;
        const snapshot = await client.inspection.snapshot(view.game.value, 'worlds');
        const data = snapshot.data as { items?: { id?: string; folder_name?: string; name?: string }[] };
        for (const world of data.items ?? []) {
            const id = world.folder_name ?? world.id;
            if (id) option(view.world, id, world.name ?? id);
        }
        if (selected && !Array.from(view.world.options).some(item => item.value === selected)) option(view.world, selected, selected);
        view.world.value = selected;
    }
    async function load(selected = '') {
        await repository.loadIndex(scope());
        draft = null; dirty = false;
        listOptions();
        const id = selected || repository.index.plans[0]?.id;
        if (id) { view.plans.value = id; draft = await repository.loadPlan(id); }
        listOptions(); rows(); await protect();
    }
    async function start() {
        context = await hostContext(client.context()); zh = context.locale === 'zh-CN';
        document.documentElement.lang = context.locale;
        document.documentElement.dataset.theme = context.theme;
        host.className = 'workbench material-planner'; view = createView(host, tr);
        option(view.game, '', tr('General plans', '通用计划'));
        for (const game of context.inspectionGames ?? context.games) option(view.game, game.id, game.name);
        if (context.instanceId) {
            if (!Array.from(view.game.options).some(item => item.value === context.instanceId)) option(view.game, context.instanceId, context.instanceId);
            view.game.value = context.instanceId;
        }
        view.name.oninput = () => { if (draft) { draft.name = view.name.value; edited(); } };
        view.search.oninput = rows; view.sort.onchange = rows;
        view.apply.onclick = () => {
            const value = stackSize(view.stack.value);
            if (!value || !draft) { view.stack.reportValidity(); return; }
            for (const item of draft.items) if (item.stackSize === null) item.stackSize = value;
            edited(); rows();
        };
        view.save.onclick = () => void run(async () => {
            if (!draft || !view.name.reportValidity() || !Array.from(view.body.querySelectorAll('input')).every(input => input.reportValidity())) return;
            draft.name = view.name.value.trim();
            if (!draft.name) { view.name.focus(); return; }
            if (!draft.id) draft.id = newId();
            draft = await repository.save(draft); dirty = false; view.error.hidden = true;
            listOptions(); rows(); await protect(); view.status.textContent = tr('Progress saved.', '进度已保存。');
        });
        view.reload.onclick = () => void run(async () => {
            await repository.loadIndex(scope());
            if (draft?.id) {
                try { await repository.loadPlan(draft.id); } catch (cause) { if (!/INCOMPLETE/.test(String(cause))) throw cause; }
            }
            if (!draft) await load(view.plans.value);
            listOptions(); view.error.hidden = true;
        });
        view.plans.onchange = () => {
            const next = view.plans.value; view.plans.value = draft?.id ?? '';
            void leave(async () => { draft = next ? await repository.loadPlan(next) : null; dirty = false; listOptions(); rows(); await protect(); });
        };
        view.game.onchange = () => {
            const next = view.game.value; view.game.value = repository.scope.gameId ?? '';
            void leave(async () => { view.game.value = next; await worlds(); await load(); });
        };
        view.world.onchange = () => {
            const next = view.world.value; view.world.value = repository.scope.worldId ?? '';
            void leave(async () => { view.world.value = next; await load(); });
        };
        view.remove.onclick = () => {
            const id = draft?.id || view.plans.value;
            if (!id) return;
            pending = async () => { await repository.remove(id); await load(); };
            view.confirm.hidden = false; view.confirmText.textContent = tr('Delete this saved plan?', '删除此计划？'); view.accept.textContent = tr('Delete plan', '删除计划');
        };
        view.accept.onclick = () => { const action = pending; pending = null; view.confirm.hidden = true; if (action) void run(action); };
        view.cancel.onclick = () => { pending = null; view.confirm.hidden = true; };
        view.copy.onclick = () => void run(async () => {
            if (!draft) return;
            const text = planText(draft, zh);
            if (new TextEncoder().encode(text).length > 8192) { view.status.textContent = tr('This list is too long for the clipboard. Export text for the complete list.', '清单超出剪贴板长度，请导出完整文本。'); return; }
            try { await client.clipboard.writeText(text); view.status.textContent = tr('List copied.', '清单已复制。'); }
            catch { view.manualCopy.hidden = false; view.manualCopy.value = text; view.manualCopy.focus(); view.manualCopy.select(); view.status.textContent = tr('Select and copy the text below.', '请选择并复制下方文本。'); }
        });
        view.download.onclick = () => void run(async () => {
            if (!draft) return;
            const result = await client.exports.text({ name: exportName(draft), text: planText(draft, zh) });
            view.status.textContent = result.saved ? tr('Text exported.', '文本已导出。') : '';
        });
        await run(async () => {
            const handoff = await client.handoffs.take({ interfaceId: MATERIALS_IMPORT_INTERFACE, version: MATERIALS_IMPORT_VERSION });
            if (handoff) {
                const gameId = handoff.game?.id ?? '';
                if (gameId && !Array.from(view.game.options).some(item => item.value === gameId)) option(view.game, gameId, handoff.game!.name);
                view.game.value = gameId;
                await worlds(handoff.worldId ?? ''); await repository.loadIndex(scope());
                draft = importedPlan(handoff.payload); dirty = true; listOptions(); rows(); await protect();
            } else { await worlds(); await load(); }
        });
    }
    void start().catch(cause => { if (view) fail(cause); else host.textContent = 'Could not open Material Planner / 无法打开材料筹备清单'; });
}
try { mountMaterialPlanner(connect()); }
catch { document.getElementById('app')!.textContent = 'Open Material Planner from Cobble Launcher / 请从 Cobble Launcher 打开材料筹备清单'; }

installPluginPresentation(presentation);
