// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext, connect, WAYPOINT_IMPORT_INTERFACE, WAYPOINT_IMPORT_VERSION, type Client, type Context, type DataScope, type WaypointHandoffDraft, listServerContexts } from '../../../../packages/plugin-web-sdk/src/index';
import { NotesEditor } from './notes-model';
import { createNotesView } from './notes-view';
import { BookEditor } from './editor';
import { createPortalView } from './portal-view';
import { coordinate, coordinates, converted, emptyDraft, locationOf, visibleLocations, type Draft } from './model';
import { createView, button, el, option } from './view';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
export function mountCoordinateBook(client: Client, host = document.getElementById('app')!) {
    host.className = 'workbench coordinate-book';
    let context: Context | null = null, zh = false, busy = false, disposed = false, scopeEpoch = 0;
    let pending: (() => Promise<void> | void) | null = null;
    let pendingDelete = false;
    const editor = new BookEditor(client.data);
    const notes = new NotesEditor(client.data);
    let notesView!: ReturnType<typeof createNotesView>;
    const tr = (en: string, cn: string) => zh ? cn : en;
    let view!: ReturnType<typeof createView>;
    let portal!: ReturnType<typeof createPortalView>;
    let activeView: 'book' | 'planner' | 'notes' = 'book';
    let portalDirection: 'overworld-to-nether' | 'nether-to-overworld' = 'overworld-to-nether';
    let portalOrigin = '', portalTarget = '', portalSearch = '';
    let guardTail: Promise<unknown> = Promise.resolve();
    const dimensionName = (value: string) => ({ overworld: tr('Overworld', '主世界'), nether: tr('Nether', '下界'), end: tr('End', '末地') } as Record<string, string>)[value] ?? value;
    function fail(cause: unknown) {
        const code = String(cause);
        view.error.hidden = false;
        view.reload.hidden = false;
        view.errorText.textContent = /CONFLICT/.test(code) ? tr('Saved data changed. Reload to keep editing your draft.', '已保存的数据发生变化，请重新加载后继续编辑草稿。') : /PERMISSION|REVOKED/.test(code) ? tr('Allow private storage and world access in this tool’s permissions.', '请在此工具权限中允许私有存储与世界读取。') : /LIMIT|TOO_LARGE|QUOTA/.test(code) ? tr('This coordinate book is full. Remove an entry or shorten its notes.', '坐标簿已满，请删除条目或缩短笔记。') : /DOCUMENT_INVALID/.test(code) ? tr('The saved coordinate book is not supported. Your draft has not been saved.', '无法读取此坐标簿格式，草稿尚未保存。') : tr('Could not save or load this book. Your draft is unchanged.', '无法保存或加载坐标簿，草稿未更改。');
    }
    function guard() {
        const dirty = editor.dirty || notes.dirty;
        guardTail = guardTail.catch(() => {
        }).then(() => client.view.setDirty({ dirty }));
        void guardTail.catch(() => {
            if (!disposed) {
                view.error.hidden = false;
                view.errorText.textContent = tr('Could not protect unsaved edits. Keep this tool open and try again.', '无法保护未保存的修改，请保持工具打开并重试。');
            }
        });
        return guardTail;
    }
    function scope(): DataScope {
        const value = view.world.value;
        return { gameId: view.game.value, ...(value.startsWith('server:') ? { serverId: value.slice(7) } : value ? { worldId: value } : {}) };
    }
    function renderList() {
        view.list.replaceChildren();
        if (!editor.loaded) {
            view.list.append(el('p', busy ? tr('Loading…', '正在加载…') : tr('Locations unavailable.', '坐标暂不可用。'), 'empty'));
            return;
        }
        const rows = visibleLocations(editor.book, view.search.value);
        if (!rows.length)
            view.list.append(el('p', view.search.value ? tr('No matching locations.', '没有匹配的地点。') : tr('No saved locations.', '还没有保存的地点。'), 'empty'));
        for (const location of rows) {
            const row = button('', () => void leaveTo(() => {
                editor.select(location.id);
                render();
                void guard();
            }), 'select', 'location-row');
            row.dataset.id = location.id;
            row.dataset.active = String(location.id === editor.draft.id);
            row.setAttribute('aria-pressed', String(location.id === editor.draft.id));
            row.append(el('strong', location.name), el('span', `${dimensionName(location.dimension)} · ${coordinates(location)}`));
            row.disabled = busy || !!pending;
            view.list.append(row);
        }
    }
    function render() {
        if (disposed)
            return;
        host.setAttribute('aria-busy', String(busy));
        view.game.disabled = busy || !!pending || !!context?.instanceId;
        view.world.disabled = busy || !!pending;
        view.add.disabled = busy || !!pending || !editor.loaded;
        view.fieldset.disabled = busy || !!pending || !editor.loaded;
        view.reload.disabled = busy || !!pending;
        const draft = editor.draft;
        for (const name of ['name', 'dimension', 'x', 'y', 'z', 'note'] as const)
            if (view[name].value !== draft[name])
                view[name].value = draft[name];
        view.dirty.hidden = !editor.dirty;
        view.save.disabled = busy || !editor.loaded || !editor.dirty || !locationOf(draft, draft.id || 'new');
        view.discard.disabled = busy || !editor.dirty;
        view.remove.hidden = !draft.id;
        view.remove.disabled = busy || !draft.id;
        view.copy.disabled = coordinate(draft.x, 'x') === null || coordinate(draft.z, 'z') === null || (draft.y !== '' && coordinate(draft.y, 'y') === null);
        view.confirmBar.hidden = !pending;
        view.layout.hidden = activeView !== 'book';
        view.bookTab.setAttribute('aria-pressed', String(activeView === 'book'));
        view.plannerTab.setAttribute('aria-pressed', String(activeView === 'planner'));
        portal.root.hidden = activeView !== 'planner';
        notesView.root.hidden = activeView !== 'notes';
        view.notesTab.setAttribute('aria-pressed', String(activeView === 'notes'));
        notesView.render(editor.book, busy || !!pending);
        portal.render(editor.book, portalOrigin, portalTarget, portalSearch, editor.dirty);
        const parsed = locationOf({ ...draft, name: draft.name || 'Location' }, draft.id || 'preview'), next = parsed ? converted(parsed) : null;
        view.conversion.hidden = !next;
        if (next && parsed) {
            view.conversionText.textContent = parsed.dimension === 'overworld' ? tr(`Nether: X ${next.x} · Z ${next.z} · round down`, `下界：X ${next.x} · Z ${next.z} · 向下取整`) : tr(`Overworld: X ${next.x} · Z ${next.z}`, `主世界：X ${next.x} · Z ${next.z}`);
            view.conversionButton.textContent = parsed.dimension === 'overworld' ? tr('Use Nether coordinates', '使用下界坐标') : tr('Use Overworld coordinates', '使用主世界坐标');
        }
        renderList();
    }
    async function leaveTo(action: () => Promise<void> | void) {
        if (busy || pending)
            return;
        await guardTail.catch(() => {
        });
        if (editor.dirty || notes.dirty) {
            pending = action;
            pendingDelete = false;
            view.confirmText.textContent = tr('Discard unsaved changes?', '放弃未保存的修改？');
            view.confirm.textContent = tr('Discard changes', '放弃修改');
            render();
            return;
        }
        await action();
    }
    async function confirmPending() {
        const action = pending;
        if (!action || busy)
            return;
        pending = null;
        if (!pendingDelete) {
            editor.select(editor.draft.id || null);
            notes.select(notes.draft.id || null);
        }
        pendingDelete = false;
        await guard();
        await action();
        render();
    }
    async function loadCurrent(preserveDraft = false) {
        if (!view.game.value) {
            editor.loaded = false;
            render();
            return;
        }
        busy = true;
        view.error.hidden = true;
        render();
        try {
            await editor.load(scope(), preserveDraft);
            await notes.load(scope(), preserveDraft);
            if (preserveDraft)
                view.status.textContent = tr('Saved data reloaded. Your draft is unchanged.', '已重新加载数据，草稿未更改。');
            await guard();
        }
        catch (cause) {
            fail(cause);
        }
        finally {
            busy = false;
            render();
        }
    }
    async function loadWorlds(worldId?: string) {
        const current = ++scopeEpoch;
        view.world.replaceChildren();
        option(view.world, '', tr('Game locations', '游戏坐标'));
        busy = true;
        render();
        try {
            const result = await client.inspection.snapshot(view.game.value, 'worlds');
            if (current !== scopeEpoch || disposed)
                return;
            const data = result.data as {
                items?: unknown[];
            };
            for (const value of data.items ?? []) {
                if (value && typeof value === 'object' && 'id' in value && typeof value.id === 'string' && 'name' in value && typeof value.name === 'string')
                    option(view.world, value.id, value.name);
            }
            try {
                const servers = await listServerContexts(client, view.game.value);
                if (current !== scopeEpoch || disposed) return;
                for (const server of servers.items) option(view.world, `server:${server.serverId}`, tr(`Server: ${server.name}`, `服务器：${server.name}`));
                if (servers.truncated) view.status.textContent = tr('Some saved servers are outside the listing limit.', '部分服务器超出列表上限。');
            } catch {
                view.status.textContent = tr('Saved servers are unavailable.', '已保存的服务器暂不可用。');
            }
            if (worldId) {
                if (!Array.from(view.world.options).some(item => item.value === worldId))
                    throw Error('PLUGIN_WORLD_SCOPE');
                view.world.value = worldId;
            }
        }
        catch (cause) {
            fail(cause);
            if (worldId) {
                busy = false;
                render();
                return false;
            }
        }
        busy = false;
        await loadCurrent();
        return true;
    }
    async function noteOperation(run: () => Promise<void>, message: string) {
        if (busy || !notes.loaded) return;
        busy = true; view.error.hidden = true; render();
        try { await guard(); await run(); view.status.textContent = message; await guard(); }
        catch (cause) { fail(cause); }
        finally { busy = false; render(); }
    }
    async function save() {
        if (busy || !editor.loaded)
            return;
        await guard();
        busy = true;
        view.error.hidden = true;
        render();
        try {
            await editor.save();
            view.status.textContent = tr('Location saved.', '地点已保存。');
            await guard();
        }
        catch (cause) {
            fail(cause);
        }
        finally {
            busy = false;
            render();
        }
    }
    function requestRemove() {
        if (busy || !editor.draft.id)
            return;
        pendingDelete = true;
        pending = async () => {
            busy = true;
            view.error.hidden = true;
            render();
            try {
                await editor.remove();
                view.status.textContent = tr('Location deleted.', '地点已删除。');
                await guard();
            }
            catch (cause) {
                fail(cause);
            }
            finally {
                busy = false;
                render();
            }
        };
        view.confirmText.textContent = tr(`Delete “${editor.draft.name}”?`, `删除“${editor.draft.name}”？`);
        view.confirm.textContent = tr('Delete', '删除');
        render();
    }
    async function copy() {
        const x = coordinate(editor.draft.x, 'x'), z = coordinate(editor.draft.z, 'z'), y = editor.draft.y === '' ? null : coordinate(editor.draft.y, 'y');
        if (x === null || z === null || (editor.draft.y !== '' && y === null))
            return;
        const value = coordinates({ x, y, z });
        try {
            await client.clipboard.writeText(value);
            view.status.textContent = tr('Coordinates copied.', '坐标已复制。');
        }
        catch {
            const selected = el('input');
            selected.readOnly = true;
            selected.value = value;
            selected.setAttribute('aria-label', tr('Select and copy coordinates', '选择并复制坐标'));
            view.status.replaceChildren(document.createTextNode(tr('Copy: ', '复制：')), selected);
            selected.focus();
            selected.select();
        }
    }
    function convert() {
        const value = locationOf({ ...editor.draft, name: editor.draft.name || 'Location' }, editor.draft.id || 'preview');
        const next = value ? converted(value) : null;
        if (!next)
            return;
        editor.edit({ dimension: next.dimension, x: String(next.x), z: String(next.z) });
        render();
        void guard();
    }
    async function receive(draft: WaypointHandoffDraft) {
        if (context?.instanceId && draft.game.id !== context.instanceId)
            throw Error('PLUGIN_GAME_SCOPE');
        if (!Array.from(view.game.options).some(item => item.value === draft.game.id))
            throw Error('PLUGIN_GAME_SCOPE');
        view.game.value = draft.game.id;
        if (!await loadWorlds(draft.worldId ?? undefined))
            return;
        const incoming: Draft = { ...emptyDraft(), name: draft.payload.name, dimension: draft.payload.dimension, x: String(draft.payload.x), y: draft.payload.y === undefined ? '' : String(draft.payload.y), z: String(draft.payload.z), note: draft.payload.source ?? '' };
        if (!locationOf(incoming, 'handoff'))
            throw Error('BOOK_DRAFT_INVALID');
        editor.prefill(incoming);
        view.status.textContent = tr(`From ${draft.sender.name} · not saved yet`, `来自 ${draft.sender.name} · 尚未保存`);
        render();
        await guard();
    }
    async function start() {
        try {
            context = await hostContext(client.context());
            zh = context.locale === 'zh-CN';
            document.documentElement.lang = zh ? 'zh-CN' : 'en';
            document.documentElement.dataset.theme = context.theme === 'system' ? (matchMedia('(prefers-color-scheme:dark)').matches ? 'dark' : 'light') : context.theme;
            view = createView(host, tr, { showBook: () => { activeView = 'book'; render(); }, showPlanner: () => { activeView = 'planner'; render(); }, showNotes: () => { activeView = 'notes'; render(); }, new: () => void leaveTo(() => {
                    editor.select(null);
                    render();
                    void guard();
                }), save: () => void save(), discard: () => {
                    editor.select(editor.draft.id || null);
                    view.error.hidden = true;
                    render();
                    void guard();
                }, remove: requestRemove, copy: () => void copy(), reload: () => void loadCurrent(true), convert, confirm: () => void confirmPending(), cancel: () => {
                    pending = null;
                    render();
                } });
            portal = createPortalView(zh, {
                direction: value => { portalDirection = value; portalOrigin = ''; portalTarget = ''; render(); },
                origin: value => { portalOrigin = value; portalTarget = ''; render(); },
                search: value => { portalSearch = value; render(); },
                select: value => { portalTarget = value; render(); },
                open: location => void leaveTo(async () => {
                    editor.select(location.id);
                    activeView = 'book';
                    render();
                    await guard();
                }),
            });
            portal.direction.value = portalDirection;
            notesView = createNotesView(notes, tr, {
                changed: () => { render(); void guard(); },
                leave: run => void leaveTo(run),
                save: () => void noteOperation(() => notes.save(), tr('Note saved.', '笔记已保存。')),
                remove: () => {
                    if (busy || !notes.draft.id) return;
                    pendingDelete = true;
                    pending = () => noteOperation(() => notes.remove(), tr('Note deleted.', '笔记已删除。'));
                    view.confirmText.textContent = tr(`Delete “${notes.draft.title}”?`, `删除“${notes.draft.title}”？`);
                    view.confirm.textContent = tr('Delete note', '删除笔记'); render();
                },
                openLocation: id => void leaveTo(() => { editor.select(id); activeView = 'book'; render(); void guard(); }),
            });
            host.append(portal.root, notesView.root);
            for (const game of context.inspectionGames ?? context.games) {
                if (!context.instanceId || context.instanceId === game.id)
                    option(view.game, game.id, game.name);
            }
            view.game.hidden = !!context.instanceId;
            view.gameText.hidden = !context.instanceId;
            view.gameText.textContent = view.game.selectedOptions[0]?.textContent ?? '';
            for (const name of ['name', 'dimension', 'x', 'y', 'z', 'note'] as const)
                view[name].addEventListener(name === 'dimension' ? 'change' : 'input', () => {
                    if (busy)
                        return;
                    editor.edit({ [name]: view[name].value } as Partial<Draft>);
                    view.status.textContent = '';
                    render();
                    void guard();
                });
            view.search.oninput = renderList;
            view.game.onchange = () => {
                const next = view.game.value;
                view.game.value = editor.scope.gameId ?? '';
                void leaveTo(async () => {
                    view.game.value = next;
                    await loadWorlds();
                });
            };
            view.world.onchange = () => {
                const next = view.world.value;
                view.world.value = editor.scope.worldId ?? (editor.scope.serverId ? `server:${editor.scope.serverId}` : '');
                void leaveTo(async () => {
                    view.world.value = next;
                    await loadCurrent();
                });
            };
            if (view.game.value) {
                const handoff = await client.handoffs.take({ interfaceId: WAYPOINT_IMPORT_INTERFACE, version: WAYPOINT_IMPORT_VERSION });
                if (handoff)
                    await receive(handoff);
                else
                    await loadWorlds();
            }
            else
                render();
        }
        catch (cause) {
            if (view)
                fail(cause);
            else
                host.textContent = tr('Could not open Coordinate Book.', '无法打开坐标簿。');
        }
    }
    void start();
    return { dispose: () => {
            disposed = true;
            scopeEpoch++;
        } };
}
try {
    mountCoordinateBook(connect());
}
catch {
    const host = document.getElementById('app');
    if (host)
        host.textContent = 'Open Coordinate Book from Cobble Launcher / 请从 Cobble Launcher 打开坐标簿';
}

installPluginPresentation(presentation);
