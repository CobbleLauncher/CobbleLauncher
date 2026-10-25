// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext, connect, type Client, type Context, type Report } from '../../../../packages/plugin-web-sdk/src/index';
import { object, orderedPair, pageRange, size, snapshotTime, toggleSnapshot, worldRows } from './model';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
type Snapshot = Awaited<ReturnType<Client['worldSnapshots']['list']>>[number];
type Comparison = Awaited<ReturnType<Client['worldSnapshots']['compare']>>;
export function mountTimeMachine(client: Client, host = document.getElementById('app')!) {
    host.className = 'workbench time-machine';
    let context: Context | null = null, zh = false, generation = 0, reportGeneration = 0, busy = false, recovering = false, checking = false, snapshots: Snapshot[] = [], selected: string[] = [], activeId = '', mode: 'snapshot' | 'compare' = 'snapshot', comparison: Comparison | null = null, query = '', offset = 0, report: Report | null = null;
    let snapshotState: 'idle' | 'loading' | 'ready' | 'error' = 'idle', compareState: 'idle' | 'loading' | 'ready' | 'error' = 'idle', analysisMessage = '';
    let searchTimer: ReturnType<typeof setTimeout> | undefined;
    const limit = 50, tr = (en: string, cn: string) => zh ? cn : en;
    const el = <K extends keyof HTMLElementTagNameMap>(tag: K, text = '', className = '') => { const value = document.createElement(tag); value.textContent = text; value.className = className; return value; };
    const button = (text: string, action: () => void, className = '') => { const value = el('button', text, className); value.type = 'button'; value.onclick = action; return value; };
    const date = (value: string | number) => Number.isFinite(snapshotTime(value)) ? new Date(snapshotTime(value)).toLocaleString(zh ? 'zh-CN' : 'en-GB', { year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) : '—';
    const header = el('header', '', 'toolbar'), title = el('h1'), actions = el('div', '', 'actions'), refresh = button('', () => void loadGame()), cancel = button('', cancelWork), contexts = el('div', '', 'context'), game = el('select'), world = el('select'), gameLabel = el('label'), worldLabel = el('label'), status = el('p', '', 'status'), error = el('p', '', 'error');
    actions.append(refresh, cancel);
    header.append(title, actions);
    gameLabel.append(game);
    worldLabel.append(world);
    contexts.append(gameLabel, worldLabel);
    status.role = 'status';
    status.setAttribute('aria-live', 'polite');
    error.role = 'alert';
    error.hidden = true;
    refresh.dataset.action='refresh';
    const layout = el('div', '', 'time-layout'), timeline = el('section', '', 'inventory'), timelineHead = el('div', '', 'section-heading timeline-header'), timelineTitle = el('div'), timelineHeading = el('h2'), timelineCount = el('span', '', 'muted'), timelineSearch = el('input', '', 'timeline-search'), list = el('div', '', 'snapshot-list'), timelineActions = el('div', '', 'timeline-actions'), compare = button('', () => void loadComparison(0), 'primary'), clear = button('', () => { selected = []; renderTimeline(); }), detail = el('section', '', 'inventory'), detailBody = el('div', '', 'detail-body');
    timelineSearch.type = 'search';
    timelineTitle.append(timelineHeading, timelineCount);
    timelineHead.append(timelineTitle, timelineSearch);
    timelineActions.append(compare, clear);
    timeline.append(timelineHead, list, timelineActions);
    detail.append(detailBody);
    layout.append(timeline, detail);
    const analysis = el('details', '', 'world-check'), analysisTitle = el('summary'), analysisActions = el('div', '', 'actions'), runCheck = button('', () => void checkWorlds()), analysisStatus = el('p', '', 'status'), findings = el('div');
    analysisActions.append(runCheck);
    analysis.append(analysisTitle, analysisActions, analysisStatus, findings);
    host.replaceChildren(header, contexts, error, status, layout, analysis);
    const labels = () => { title.textContent = tr('World Time Machine', '世界时光机'); gameLabel.firstChild?.nodeType === Node.TEXT_NODE && gameLabel.firstChild.remove(); worldLabel.firstChild?.nodeType === Node.TEXT_NODE && worldLabel.firstChild.remove(); gameLabel.prepend(document.createTextNode(tr('Game', '游戏'))); worldLabel.prepend(document.createTextNode(tr('World', '世界'))); game.setAttribute('aria-label', tr('Game', '游戏')); world.setAttribute('aria-label', tr('World', '世界')); refresh.textContent = tr('Refresh', '刷新'); cancel.textContent = tr('Cancel', '取消'); timelineHeading.textContent = tr('Snapshots', '快照'); timelineSearch.placeholder = tr('Find snapshots', '查找快照'); timelineSearch.setAttribute('aria-label', timelineSearch.placeholder); compare.textContent = tr(`Compare selected (${selected.length}/2)`, `对比所选（${selected.length}/2）`); clear.textContent = tr('Clear selection', '清除选择'); analysisTitle.textContent = tr('World check · all worlds', '世界检查 · 全部世界'); runCheck.textContent = tr('Run check', '运行检查'); ui(); };
    function ui() { host.setAttribute('aria-busy', String(busy)); game.disabled = busy || !!context?.instanceId; world.disabled = busy || !world.options.length; refresh.disabled = busy || !game.value; cancel.hidden = recovering || (!busy && !checking); compare.disabled = busy || selected.length !== 2; clear.disabled = busy || selected.length === 0; runCheck.disabled = busy || checking || !game.value; analysisStatus.textContent = checking ? tr('Checking world metadata…', '正在检查世界信息…') : analysisMessage; }
    function fail(cause: unknown) { const code = String(cause); error.textContent = /PERMISSION|permission/i.test(code) ? tr('Allow world snapshot access in this tool’s permissions.', '请在此工具的权限中允许读取世界快照。') : /running|stop.*game|正在运行|停止游戏/i.test(code) ? tr('Stop this game before inspecting snapshots.', '请先停止游戏，再查看快照。') : /CHANGED|STALE|revision/i.test(code) ? tr('World data changed. Refresh and try again.', '世界数据已更改，请刷新后重试。') : tr('Could not read world snapshots. Try again.', '无法读取世界快照，请重试。'); error.hidden = false; }
    function cancelWork() { if (recovering) return; generation++; reportGeneration++; busy = false; checking = false; if (searchTimer)
        clearTimeout(searchTimer); void client.report.cancel().catch(() => { }); if (snapshotState === 'loading')
        snapshotState = 'idle'; if (compareState === 'loading')
        compareState = 'idle'; analysisMessage = tr('Check cancelled.', '检查已取消。'); status.textContent = tr('Cancelled.', '已取消。'); ui(); renderTimeline(); renderDetail(); }
    function renderTimeline() {
        timelineCount.textContent = String(snapshots.length);
        compare.textContent = tr(`Compare selected (${selected.length}/2)`, `对比所选（${selected.length}/2）`);
        list.replaceChildren();
        const needle = timelineSearch.value.trim().toLocaleLowerCase();
        const visible = snapshots.filter(snapshot => `${snapshot.name} ${date(snapshot.created_at)}`.toLocaleLowerCase().includes(needle));
        if (!visible.length)
            list.append(el('p', snapshotState === 'loading' ? tr('Reading snapshots…', '正在读取快照…') : snapshotState === 'error' ? tr('Snapshots unavailable.', '快照暂不可用。') : snapshotState === 'idle' ? tr('No snapshots loaded.', '尚未加载快照。') : snapshots.length ? tr('No matching snapshots.', '没有匹配的快照。') : tr('No snapshots for this world.', '此世界还没有快照。'), 'empty'));
        for (const snapshot of visible) {
            const row = el('article', '', 'snapshot-row');
            row.dataset.active = String(snapshot.id === activeId);
            const select = el('input');
            select.type = 'checkbox';
            select.checked = selected.includes(snapshot.id);
            select.disabled = busy || (!select.checked && selected.length === 2);
            select.setAttribute('aria-label', tr(`Compare ${snapshot.name}, ${date(snapshot.created_at)}`, `对比 ${snapshot.name}，${date(snapshot.created_at)}`));
            select.onchange = () => { selected = toggleSnapshot(selected, snapshot.id); mode = 'snapshot'; comparison = null; query = ''; renderTimeline(); renderDetail(); };
            const pick = button('', () => { activeId = snapshot.id; mode = 'snapshot'; renderTimeline(); renderDetail(); }, 'snapshot-pick');
            pick.append(el('strong', snapshot.name || date(snapshot.created_at)), el('span', date(snapshot.created_at)), el('span', size(snapshot.processed_bytes)));
            const flags = el('div', '', 'snapshot-flags');
            if (snapshot.protected)
                flags.append(el('span', tr('Protected', '受保护')));
            if (snapshot.automatic)
                flags.append(el('span', tr('Automatic', '自动')));
            pick.append(flags);
            pick.setAttribute('aria-pressed', String(activeId === snapshot.id && mode === 'snapshot'));
            row.append(select, pick);
            list.append(row);
        }
        ui();
    }
    async function requestRecovery(snapshot: Snapshot) { if (busy)
        return; busy = true; recovering = true; error.hidden = true; renderDetail(); ui(); try {
        await client.worldSnapshots.openRestore(game.value, world.value, snapshot.id);
        status.textContent = tr('Recovery request sent.', '已发送恢复请求。');
    }
    catch (cause) {
        fail(cause);
    }
    finally {
        busy = false; recovering = false;
        renderDetail(); ui();
    } }
    function renderDetail() { if (mode === 'compare') {
        renderComparison();
        return;
    } detailBody.replaceChildren(); const snapshot = snapshots.find(item => item.id === activeId); if (!snapshot) {
        detailBody.append(el('p', tr('No snapshot selected.', '未选择快照。'), 'empty'));
        return;
    } const box = el('div', '', 'snapshot-info'), heading = el('div', '', 'snapshot-title'); heading.append(el('h2', snapshot.name || date(snapshot.created_at))); box.append(heading, el('p', date(snapshot.created_at), 'snapshot-date')); const metrics = el('div', '', 'snapshot-metrics'); for (const [label, value] of [[tr('World data', '世界数据'), size(snapshot.processed_bytes)], [tr('New data', '新增数据'), size(snapshot.added_bytes)]]) {
        const metric = el('div');
        metric.append(el('strong', value), el('span', label));
        metrics.append(metric);
    } const recovery = button(tr('Review recovery', '查看恢复选项'), () => void requestRecovery(snapshot), 'primary recovery-action'); recovery.disabled = recovering; box.append(metrics, recovery); detailBody.append(box); }
    function renderComparison() {
        const focused = document.activeElement?.id === 'diff-search';
        detailBody.replaceChildren();
        const pair = orderedPair(snapshots, selected);
        const head = el('div', '', 'compare-header');
        head.append(button(tr('← Snapshots', '← 返回快照'), () => { generation++; busy = false; mode = 'snapshot'; renderTimeline(); renderDetail(); status.textContent = ''; }));
        head.append(el('h2', pair ? `${pair[0].name} → ${pair[1].name}` : tr('Compare snapshots', '对比快照')));
        detailBody.append(head);
        if (!comparison) {
            detailBody.append(el('p', compareState === 'loading' ? tr('Comparing snapshots…', '正在对比快照…') : compareState === 'error' ? tr('Comparison unavailable.', '对比暂不可用。') : tr('Comparison cancelled.', '已取消对比。'), 'empty'));
            return;
        }
        const stats = el('div', '', 'comparison-stats');
        for (const [label, value] of [[tr('Added', '新增'), comparison.added], [tr('Modified', '修改'), comparison.modified], [tr('Deleted', '删除'), comparison.deleted]]) {
            const metric = el('span', label as string);
            metric.prepend(el('strong', String(value)));
            stats.append(metric);
        }
        detailBody.append(stats);
        const toolbar = el('div', '', 'diff-toolbar'), search = el('input');
        search.id = 'diff-search';
        search.type = 'search';
        search.maxLength = 120;
        search.placeholder = tr('Search changed files', '搜索变更文件');
        search.setAttribute('aria-label', search.placeholder);
        search.value = query;
        search.oninput = () => { query = search.value; if (searchTimer)
            clearTimeout(searchTimer); searchTimer = setTimeout(() => { if (mode === 'compare')
            void loadComparison(0); }, 250); };
        toolbar.append(search, el('span', tr(`${comparison.total} matching`, `${comparison.total} 项匹配`), 'muted'));
        detailBody.append(toolbar);
        const scroll = el('div', '', 'table-scroll'), table = el('table', '', 'changed-table'), thead = el('thead'), headerRow = el('tr');
        for (const label of [tr('File', '文件'), tr('Change', '变更'), tr('Before', '之前'), tr('After', '之后')]) {
            const cell = el('th', label);
            cell.scope = 'col';
            headerRow.append(cell);
        }
        thead.append(headerRow);
        const body = el('tbody');
        for (const item of comparison.rows) {
            const row = el('tr');
            const kind = tr(item.kind === 'added' ? 'Added' : item.kind === 'modified' ? 'Modified' : 'Deleted', item.kind === 'added' ? '新增' : item.kind === 'modified' ? '修改' : '删除');
            row.append(el('td', item.path), el('td', kind, `change-${item.kind}`), el('td', size(item.before_bytes)), el('td', size(item.after_bytes)));
            body.append(row);
        }
        table.append(thead, body);
        scroll.append(table);
        detailBody.append(scroll);
        if (!comparison.rows.length)
            detailBody.append(el('p', query ? tr('No changed files match this search.', '没有匹配的变更文件。') : tr('These snapshots have identical files.', '这两个快照的文件一致。'), 'empty'));
        const pages = el('div', '', 'pagination'), previous = button(tr('Previous', '上一页'), () => void loadComparison(Math.max(0, offset - limit))), next = button(tr('Next', '下一页'), () => void loadComparison(offset + limit));
        previous.disabled = busy || offset === 0;
        next.disabled = busy || offset + comparison.rows.length >= comparison.total;
        pages.append(previous, el('span', pageRange(offset, comparison.rows.length, comparison.total)), next);
        detailBody.append(pages);
        if (focused) {
            search.focus();
            search.setSelectionRange(search.value.length, search.value.length);
        }
    }
    async function loadComparison(nextOffset: number) { const pair = orderedPair(snapshots, selected); if (!pair)
        return; const epoch = ++generation; offset = nextOffset; mode = 'compare'; compareState = 'loading'; busy = true; error.hidden = true; status.textContent = tr('Comparing snapshots…', '正在对比快照…'); ui(); renderDetail(); try {
        const result = await client.worldSnapshots.compare(game.value, world.value, pair[0].id, pair[1].id, offset, limit, query.trim());
        if (epoch !== generation)
            return;
        comparison = result;
        compareState = 'ready';
        status.textContent = '';
    }
    catch (cause) {
        if (epoch === generation) {
            comparison = null;
            compareState = 'error';
            fail(cause);
            status.textContent = '';
        }
    }
    finally {
        if (epoch === generation) {
            busy = false;
            ui();
            renderDetail();
        }
    } }
    async function loadWorld() { const epoch = ++generation; snapshotState = 'loading'; busy = true; snapshots = []; selected = []; activeId = ''; comparison = null; mode = 'snapshot'; error.hidden = true; status.textContent = tr('Reading snapshots…', '正在读取快照…'); renderTimeline(); renderDetail(); try {
        if (world.value) {
            const result = await client.worldSnapshots.list(game.value, world.value);
            if (epoch !== generation)
                return;
            snapshots = [...result].sort((a, b) => snapshotTime(b.created_at) - snapshotTime(a.created_at) || a.id.localeCompare(b.id));
            activeId = snapshots[0]?.id ?? '';
        }
        snapshotState = 'ready';
        status.textContent = '';
    }
    catch (cause) {
        if (epoch === generation) {
            snapshotState = 'error';
            fail(cause);
            status.textContent = '';
        }
    }
    finally {
        if (epoch === generation) {
            busy = false;
            renderTimeline();
            renderDetail();
        }
    } }
    async function checkWorlds(snapshot?: Awaited<ReturnType<Client['inspection']['snapshot']>>) { const epoch = ++reportGeneration, gameId = game.value; if (!gameId)
        return; checking = true; ui(); try {
        const input = snapshot ?? await client.inspection.snapshot(gameId, 'worlds');
        const result = await client.inspection.check(gameId, 'worlds', input.revision);
        if (epoch !== reportGeneration || gameId !== game.value)
            return;
        report = result;
        renderFindings();
    }
    catch {
        if (epoch === reportGeneration)
            analysisMessage = tr('World check unavailable. Try again.', '世界检查暂不可用，请重试。');
    }
    finally {
        if (epoch === reportGeneration) {
            checking = false;
            ui();
        }
    } }
    function renderFindings() { findings.replaceChildren(); for (const finding of report?.findings ?? []) {
        const box = el('article', '', 'finding');
        box.dataset.severity = finding.severity;
        box.append(el('h3', finding.title), el('p', finding.message));
        findings.append(box);
    } analysisMessage = report ? (report.findings.length ? '' : tr('No metadata issues found.', '未发现世界信息问题。')) : tr('Check cancelled.', '检查已取消。'); }
    async function loadGame() { const epoch = ++generation; reportGeneration++; checking = false; snapshotState = 'idle'; analysisMessage = ''; busy = true; report = null; findings.replaceChildren(); world.replaceChildren(); snapshots = []; selected = []; activeId = ''; comparison = null; mode = 'snapshot'; error.hidden = true; status.textContent = tr('Reading worlds…', '正在读取世界…'); renderTimeline(); renderDetail(); try {
        const input = await client.inspection.snapshot(game.value, 'worlds');
        if (epoch !== generation)
            return;
        const data = object(input.data);
        if (data.unavailable)
            throw Error('WORLD_UNAVAILABLE');
        for (const item of worldRows(input.data)) {
            const option = el('option', item.name);
            option.value = item.id;
            world.append(option);
        }
        void checkWorlds(input);
        if (!world.options.length) {
            busy = false;
            status.textContent = tr('No worlds in this game.', '此游戏还没有世界。');
            ui();
            return;
        }
        await loadWorld();
    }
    catch (cause) {
        if (epoch === generation) {
            busy = false;
            fail(cause);
            status.textContent = '';
            ui();
        }
    } }
    game.onchange = () => void loadGame();
    world.onchange = () => void loadWorld();
    timelineSearch.oninput = renderTimeline;
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') {
        if (busy || checking)
            cancelWork();
        else if (mode === 'compare') {
            mode = 'snapshot';
            renderDetail();
        }
    } };
    host.addEventListener('keydown', escape);
    async function start() { try {
        context = await hostContext(client.context());
        zh = context.locale === 'zh-CN';
        document.documentElement.lang = zh ? 'zh-CN' : 'en';
        document.documentElement.dataset.theme = context.theme === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : context.theme;
        const games = context.inspectionGames ?? context.games;
        for (const item of games.filter(item => !context!.instanceId || item.id === context!.instanceId)) {
            const option = el('option', item.name);
            option.value = item.id;
            game.append(option);
        }
        labels();
        if (game.options.length)
            await loadGame();
        else {
            status.textContent = tr('No available games.', '没有可用的游戏。');
            ui();
        }
    }
    catch (cause) {
        labels();
        fail(cause);
    } }
    void start();
    return { cancel: cancelWork };
}
try {
    mountTimeMachine(connect());
}
catch {
    const host = document.getElementById('app');
    if (host) {
        host.className = 'workbench';
        const message = document.createElement('p');
        message.setAttribute('role', 'alert');
        message.textContent = 'Open World Time Machine from the launcher. / 请从启动器打开世界时光机。';
        host.append(message);
    }
}

installPluginPresentation(presentation);
