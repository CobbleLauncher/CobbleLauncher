// SPDX-License-Identifier: MIT
import { connect, type Client, type Context, type InspectionScope, type InspectionSnapshot, type Report } from '../../plugin-web-sdk/src/index';
import { columns, datasets, filterRows, format, humanize, localized, lookup, type Field, type Localized, type Row } from './model';
import './style.css';
export type WorkbenchOptions = {
    scope: InspectionScope;
    title: Localized;
    columns?: Field[];
    metrics?: Field[];
};
const labels: Record<string, [
    string,
    string
]> = { worlds: ['Worlds', '世界'], gameMode: ['Game mode', '游戏模式'], truncated: ['Partial inventory', '清单不完整'], addressKind: ['Address category', '地址类型'], duplicateOf: ['Duplicate of', '重复条目'], name: ['Name', '名称'], version: ['Version', '版本'], loader: ['Loader', '加载器'], disabled: ['Disabled', '已禁用'], kind: ['Type', '类型'], sizeBytes: ['Size', '大小'], packFormat: ['Pack format', '资源包格式'], lastPlayed: ['Last played', '上次游玩'], hardcore: ['Hardcore', '极限模式'], metadataError: ['Metadata error', '元数据错误'], modifiedAt: ['Modified', '修改时间'], fileCount: ['Files', '文件数'], value: ['Value', '值'], worldId: ['World', '世界'], removed: ['Removed', '已移除'], items: ['Inventory', '清单'], selectedResourcePacks: ['Selected resource packs', '已选资源包'], createdAt: ['Created', '创建时间'], valid: ['Valid', '有效'], status: ['Status', '状态'] };
const scopeLabels: Record<string, [
    string,
    string
]> = { mods: ['Mods', '模组'], packs: ['Packs', '资源包'], worlds: ['Worlds', '世界'], backups: ['Backups', '备份'], screenshots: ['Screenshots', '截图'], storage: ['Folders', '文件夹'], runtime: ['Runtime settings', '运行设置'], options: ['Options', '游戏选项'], datapacks: ['Datapacks', '数据包'], servers: ['Servers', '服务器'] };
function fieldFormat(key: string): Field['format'] { return /bytes|size/i.test(key) ? 'bytes' : /At$|lastPlayed/.test(key) ? 'date' : undefined; }
export function mountWorkbench(options: WorkbenchOptions) {
    const host = document.getElementById('app') ?? document.body.appendChild(document.createElement('main'));
    host.classList.add('workbench');
    let client: Client, context: Context, zh = false, snapshot: InspectionSnapshot | null = null, report: Report | null = null, rows: Row[] = [], generation = 0, loading = false, checking = false, dataset = 'items';
    const tr = (en: string, cn: string) => zh ? cn : en;
    const label = (key: string) => labels[key]?.[zh ? 1 : 0] ?? humanize(key);
    const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text = '', className = '') => { const item = document.createElement(tag); item.textContent = text; item.className = className; return item; };
    const gameLabel = node('label'), game = node('select'), refresh = node('button'), cancel = node('button'), toolbar = node('header', '', 'toolbar'), actions = node('div', '', 'actions');
    gameLabel.append(game);
    actions.append(refresh, cancel);
    toolbar.append(gameLabel, actions);
    const status = node('p', '', 'status'), error = node('p', '', 'error'), notice = node('p', '', 'notice'), stats = node('div', '', 'stats');
    status.setAttribute('role', 'status');
    status.setAttribute('aria-live', 'polite');
    error.setAttribute('role', 'alert');
    error.hidden = true;
    notice.hidden = true;
    const inventory = node('section', '', 'inventory'), analysis = node('section', '', 'analysis'), layout = node('div', '', 'layout');
    layout.append(inventory, analysis);
    const inventoryHeader = node('div', '', 'section-heading'), collection = node('select'), search = node('input'), count = node('span', '', 'muted');
    search.type = 'search';
    inventoryHeader.append(collection, count, search);
    const tableHost = node('div', '', 'table-scroll'), empty = node('p', '', 'empty');
    inventory.append(inventoryHeader, tableHost, empty);
    const heading = node('div', '', 'section-heading'), findingsTitle = node('h2'), severity = node('select'), findings = node('div'), analysisStatus = node('p', '', 'empty');
    heading.append(findingsTitle, severity);
    analysis.append(heading, analysisStatus, findings);
    const detail = node('details', '', 'detail'), detailTitle = node('summary'), detailList = node('dl');
    detail.append(detailTitle, detailList);
    inventory.append(detail);
    host.replaceChildren(toolbar, error, status, notice, stats, layout);
    game.id = 'game';
    refresh.id = 'refresh';
    cancel.id = 'cancel';
    tableHost.id = 'inventory';
    findings.id = 'findings';
    analysisStatus.id = 'analysis-status';
    function ui() { game.disabled = loading || !!context?.instanceId; refresh.disabled = loading || !game.value; refresh.textContent = tr('Refresh', '刷新'); cancel.textContent = tr('Cancel analysis', '取消分析'); cancel.hidden = !checking; game.setAttribute('aria-label', tr('Game', '游戏')); search.placeholder = tr('Search inventory', '搜索清单'); search.setAttribute('aria-label', search.placeholder); findingsTitle.textContent = tr('Findings', '检查结果'); severity.setAttribute('aria-label', tr('Filter findings', '筛选结果')); collection.setAttribute('aria-label', tr('Inventory', '清单')); host.setAttribute('aria-busy', String(loading)); }
    function renderStats() { stats.replaceChildren(); if (!snapshot)
        return; const items = datasets(snapshot.data).find(value => value.key === 'items')?.rows ?? []; const bytes = items.reduce((total, row) => total + (typeof row.sizeBytes === 'number' ? row.sizeBytes : 0), 0); const metrics = options.metrics ?? [{ key: '$count', label: scopeLabels[options.scope] ? [scopeLabels[options.scope]![0], scopeLabels[options.scope]![1]][zh ? 1 : 0] : tr('Items', '项目') }, ...(bytes ? [{ key: '$bytes', label: tr('Total size', '总大小'), format: 'bytes' as const }] : []), { key: '$findings', label: tr('Findings', '诊断') }]; for (const metric of metrics) {
        const value = metric.key === '$count' ? items.length : metric.key === '$bytes' ? bytes : metric.key === '$findings' ? report?.findings.length : lookup(snapshot.data, metric.key);
        const box = node('div', '', 'metric');
        box.append(node('strong', format(value, metric.format, zh)), node('span', metric.label ? localized(metric.label, zh) : label(metric.key)));
        stats.append(box);
    } }
    function showDetails(row: Row) { detailTitle.textContent = format(row.name ?? row.id ?? tr('Details', '详情')); detailList.replaceChildren(); for (const [key, value] of Object.entries(row)) {
        const term = node('dt', label(key)), description = node('dd', format(value, fieldFormat(key), zh));
        detailList.append(term, description);
    } detail.open = true; detail.hidden = false; }
    function renderRows() { const visible = filterRows(rows, search.value); count.textContent = `${visible.length} / ${rows.length}`; tableHost.replaceChildren(); empty.hidden = !!visible.length; empty.textContent = search.value ? tr('No matches.', '没有匹配项。') : tr('No items found.', '未找到项目。'); if (!visible.length)
        return; const fields = options.columns ?? columns(rows); const table = node('table'), thead = node('thead'), header = node('tr'); for (const field of fields) {
        const th = node('th', field.label ? localized(field.label, zh) : label(field.key));
        th.scope = 'col';
        header.append(th);
    } thead.append(header); table.append(thead); const body = node('tbody'); for (const row of visible) {
        const item = node('tr');
        for (const [index, field] of fields.entries()) {
            const cell = node('td');
            if (/version|loader/i.test(field.key))
                cell.classList.add('compact-value');
            const text = format(lookup(row, field.key), field.format ?? fieldFormat(field.key), zh);
            if (index === 0) {
                const button = node('button', text, 'row-action');
                button.onclick = () => showDetails(row);
                cell.append(button);
            }
            else
                cell.textContent = text;
            item.append(cell);
        }
        body.append(item);
    } table.append(body); tableHost.append(table); }
    function renderSnapshot() { if (!snapshot)
        return; const groups = datasets(snapshot.data); collection.replaceChildren(...groups.map(group => new Option(group.key === 'items' ? (scopeLabels[options.scope]?.[zh ? 1 : 0] ?? label(group.key)) : label(group.key), group.key))); if (!groups.some(group => group.key === dataset))
        dataset = groups[0]?.key ?? 'items'; collection.value = dataset; collection.hidden = groups.length < 2; rows = groups.find(group => group.key === dataset)?.rows ?? []; const data = snapshot.data as Record<string, unknown>; notice.hidden = !data?.truncated && !data?.unavailable; notice.textContent = data?.unavailable ? tr('Some information is unavailable.', '部分信息不可用。') : tr('The inventory reached its size limit. Some items are not shown.', '清单已达到数量上限，部分项目未显示。'); detail.hidden = true; renderStats(); renderRows(); }
    function renderReport() { findings.replaceChildren(); const visible = (report?.findings ?? []).filter(f => severity.value === 'all' || f.severity === severity.value); analysisStatus.hidden = !!visible.length; analysisStatus.textContent = checking ? tr('Analyzing…', '正在分析…') : report ? (report.findings.length ? tr('No findings in this filter.', '当前筛选下没有诊断。') : tr('No issues found.', '未发现问题。')) : tr('Select a game to inspect.', '选择游戏以查看。'); severity.hidden = !report?.findings.length; for (const finding of visible) {
        const card = node('article', '', 'finding');
        card.dataset.severity = finding.severity;
        card.append(node('span', tr(finding.severity === 'error' ? 'Error' : finding.severity === 'warning' ? 'Warning' : 'Information', finding.severity === 'error' ? '错误' : finding.severity === 'warning' ? '警告' : '提示'), 'badge'), node('h3', finding.title), node('p', finding.message));
        for (const suggestion of finding.suggestions)
            card.append(node('p', suggestion, 'suggestion'));
        findings.append(card);
    } for (const source of report?.tables ?? []) {
        const box = node('div', '', 'report-table table-scroll'), table = node('table'), head = node('thead'), row = node('tr');
        for (const title of source.columns) {
            const cell = node('th', title);
            cell.scope = 'col';
            row.append(cell);
        }
        head.append(row);
        table.append(head);
        const body = node('tbody');
        for (const values of source.rows) {
            const row = node('tr');
            for (const value of values)
                row.append(node('td', value));
            body.append(row);
        }
        table.append(body);
        box.append(table);
        findings.append(box);
    } renderStats(); }
    function fail(cause: unknown) { const code = String(cause); error.textContent = code.includes('PERMISSION') ? tr('Approve this tool’s game inspection permission to continue.', '请批准此工具的游戏检查权限。') : code.includes('CHANGED') ? tr('Game data changed. Refresh to analyze it again.', '游戏数据已更改，请刷新后重新分析。') : tr('Could not inspect this game. Try refreshing.', '无法检查此游戏，请尝试刷新。'); error.hidden = false; }
    async function load() { if (loading || !game.value)
        return; const epoch = ++generation; if (checking) {
        checking = false;
        await client.report.cancel().catch(() => { });
    } loading = true; report = null; error.hidden = true; status.textContent = tr('Reading game data…', '正在读取游戏数据…'); ui(); try {
        const value = await client.inspection.snapshot(game.value, options.scope);
        if (epoch !== generation)
            return;
        snapshot = value;
        renderSnapshot();
        loading = false;
        checking = true;
        status.textContent = '';
        ui();
        renderReport();
        const result = await client.inspection.check(game.value, options.scope, value.revision);
        if (epoch !== generation)
            return;
        report = result;
        checking = false;
        renderReport();
    }
    catch (cause) {
        if (epoch === generation) {
            checking = false;
            fail(cause);
        }
    }
    finally {
        if (epoch === generation) {
            loading = false;
            status.textContent = '';
            ui();
        }
    } }
    refresh.onclick = () => void load();
    game.onchange = () => { snapshot = null; report = null; rows = []; tableHost.replaceChildren(); stats.replaceChildren(); findings.replaceChildren(); detail.hidden = true; void load(); };
    search.oninput = renderRows;
    collection.onchange = () => { dataset = collection.value; renderSnapshot(); };
    severity.onchange = renderReport;
    cancel.onclick = async () => { generation++; checking = false; await client.report.cancel().catch(fail); analysisStatus.hidden = false; analysisStatus.textContent = tr('Analysis cancelled. Refresh to try again.', '已取消分析，刷新后可重试。'); ui(); };
    const controller = { refresh: () => load(), dispose: () => { generation++; if (checking)
            void client?.report.cancel().catch(() => { }); } };
    window.addEventListener('pagehide', controller.dispose, { once: true });
    void (async () => { try {
        client = connect();
        context = await client.context();
        zh = context.locale === 'zh-CN';
        document.documentElement.lang = zh ? 'zh-CN' : 'en';
        document.title = localized(options.title, zh);
        const dark = matchMedia('(prefers-color-scheme: dark)');
        const theme = () => document.documentElement.dataset.theme = context.theme === 'system' ? (dark.matches ? 'dark' : 'light') : context.theme;
        theme();
        dark.addEventListener('change', theme);
        const games = context.inspectionGames ?? context.games;
        game.replaceChildren(new Option(tr('Choose game', '选择游戏'), ''), ...games.map(value => new Option(`${value.name} · ${value.version}`, value.id)));
        game.value = context.instanceId ?? games[0]?.id ?? '';
        severity.replaceChildren(...[['all', 'All findings', '全部诊断'], ['error', 'Errors', '错误'], ['warning', 'Warnings', '警告'], ['info', 'Information', '提示']].map(([value, en, cn]) => new Option(tr(en!, cn!), value)));
        detail.hidden = true;
        ui();
        renderReport();
        if (game.value)
            await load();
        else {
            empty.textContent = tr('No registered games available.', '暂无已注册的游戏。');
        }
    }
    catch (cause) {
        fail(cause);
        ui();
    } })();
    return controller;
}
