// SPDX-License-Identifier: MIT
import type { RecoverySnapshot, RecoveryTarget } from '../../../../packages/plugin-web-sdk/src/recovery-workspace';
import { recoveryEvents } from './recovery-model';
import { comparisonChanges, comparisonRecords, type ComparisonField } from './comparison-model';
const copy = {
  en: { title: 'Recovery', loading: 'Reading recovery history…', noGame: 'Choose a game.', normal: 'Last clean exit', unknown: 'No recorded clean exit', unavailable: 'Recovery history is unavailable. Refresh to try again.', baselineUnavailable: 'The clean-exit record could not be read.', since: 'Since that launch started', all: 'All recorded history', search: 'Search changes', kinds: 'Change type', every: 'All changes', mods: 'Mods', configs: 'Configuration backups', related: 'Changes are a timeline, not proof of a cause.', runtime: 'Current runtime configuration', changed: 'Changed since that launch', same: 'Recorded settings match', java: 'Java', memory: 'Memory', arguments: 'Extra JVM arguments', inherit: 'Use launcher setting', automatic: 'Automatic', custom: 'Custom', unknownJava: 'Unknown version', runtimeOpen: 'Review runtime settings', modOpen: 'Review undo', configOpen: 'Review backup', empty: 'No recorded changes in this view.', limited: 'History is limited to retained records; showing up to 100 matches.', missingMods: 'Mod history could not be read.', missingConfigs: 'Configuration backup history could not be read.', running: 'Close the game before requesting recovery actions.', opening: 'Opening the launcher review…', opened: 'Review opened in the launcher.', changedError: 'History changed. Refresh before continuing.', failed: 'Could not open this review. Refresh and try again.', applied: 'Applied', undone: 'Undone', undoing: 'Recovery incomplete', installing: 'Installation incomplete', backup: 'Backup before a configuration write', unknownStatus: 'Unknown status' },
  'zh-CN': { title: '失败恢复', loading: '正在读取恢复记录…', noGame: '请选择游戏。', normal: '上次正常退出', unknown: '尚无正常退出记录', unavailable: '无法读取恢复记录，请刷新重试。', baselineUnavailable: '无法读取正常退出记录。', since: '从该次启动开始', all: '全部保留记录', search: '搜索变更', kinds: '变更类型', every: '全部变更', mods: 'Mods', configs: '配置备份', related: '时间上的先后不能证明变更导致了故障。', runtime: '当前运行配置', changed: '与该次启动时不同', same: '已记录的设置相同', java: 'Java', memory: '内存', arguments: '额外 JVM 参数', inherit: '使用启动器设置', automatic: '自动选择', custom: '自定义', unknownJava: '版本未知', runtimeOpen: '检查运行设置', modOpen: '预览撤销', configOpen: '查看备份', empty: '此视图中暂无变更记录。', limited: '仅包含保留记录，最多显示 100 条匹配项。', missingMods: '无法读取 Mod 操作记录。', missingConfigs: '无法读取配置备份记录。', running: '请关闭游戏后再进行恢复操作。', opening: '正在打开启动器确认页面…', opened: '已在启动器中打开确认页面。', changedError: '记录已变化，请刷新后继续。', failed: '无法打开，请刷新重试。', applied: '已应用', undone: '已撤销', undoing: '恢复未完成', installing: '安装未完成', backup: '写入配置前的备份', unknownStatus: '状态未知' },
};
type Locale = keyof typeof copy;
const comparisonCopy = {
  en: { title: 'Last clean exit → Failed attempt', noBaseline: 'No clean exit recorded yet.', noFailedAttempt: 'No failed attempt recorded yet.', baselineAfterFailure: 'The clean exit is newer than the failed attempt.', baselineUnavailable: 'The clean-exit record could not be read.', failedAttemptUnavailable: 'The failed-attempt record could not be read.', settings: 'Settings at launch', changes: 'Managed records in this interval', noChanges: 'No recorded changes in this interval.', unknown: 'Some settings were not captured for both launches.', limited: 'Some records are outside the retained history.', external: 'Edits made outside the launcher are not tracked here.', later: 'After failed attempt', allHistory: 'Show all history', laterOnly: 'Show later only', current: 'Current settings', minecraftVersion: 'Minecraft', loaderKind: 'Loader', loaderVersion: 'Loader version', javaMode: 'Java selection', javaMajor: 'Java version', memoryMb: 'Memory', jvmArgumentCount: 'JVM arguments', none: 'None', backup: 'Backup created', modStarted: 'Mod operation started', applied: 'Applied', undone: 'Undone', undoing: 'Recovery incomplete', installing: 'Installation incomplete', unknownStatus: 'Status unknown' },
  'zh-CN': { title: '上次正常退出 → 本次失败', noBaseline: '尚无正常退出记录。', noFailedAttempt: '尚无失败启动记录。', baselineAfterFailure: '正常退出记录晚于本次失败。', baselineUnavailable: '无法读取正常退出记录。', failedAttemptUnavailable: '无法读取失败启动记录。', settings: '启动时设置', changes: '此期间的托管记录', noChanges: '此期间无已记录变更。', unknown: '部分启动设置未在两次启动时完整记录。', limited: '部分记录已超出保留范围。', external: '未记录在启动器外修改的文件。', later: '失败之后', allHistory: '查看全部记录', laterOnly: '仅看失败之后', current: '当前设置', minecraftVersion: 'Minecraft', loaderKind: '加载器', loaderVersion: '加载器版本', javaMode: 'Java 选择', javaMajor: 'Java 版本', memoryMb: '内存', jvmArgumentCount: 'JVM 参数', none: '无', backup: '已创建备份', modStarted: 'Mod 操作开始', applied: '已应用', undone: '已撤销', undoing: '恢复未完成', installing: '安装未完成', unknownStatus: '状态未知' },
} as const;
type API = { snapshot: (gameId: string) => Promise<RecoverySnapshot>; open: (gameId: string, revision: string, target: RecoveryTarget) => Promise<{ requested: true }> };
function node<K extends keyof HTMLElementTagNameMap>(tag: K, text = '') { const result = document.createElement(tag); result.textContent = text; return result; }
export function createRecoveryView(root: HTMLElement, api: API) {
  let gameId = '', locale: Locale = 'en', active = false, loading = false, opening = false, snapshot: RecoverySnapshot | null = null, request = 0;
  const comparison = node('section'), runtime = node('section'), listTitle = node('h3'), controls = node('div'), search = node('input'), kind = node('select'), since = node('button'), list = node('ol'), status = node('p'), hint = node('p');
  comparison.className = 'recovery-comparison'; runtime.className = 'recovery-runtime'; controls.className = 'recovery-controls'; list.className = 'recovery-events'; status.setAttribute('role', 'status'); status.className = 'recovery-status'; hint.className = 'recovery-hint'; search.type = 'search'; search.maxLength = 120;
  const modes = ['all', 'mod', 'config'] as const; for (const value of modes) { const option = node('option'); option.value = value; kind.append(option); }
  listTitle.className = 'recovery-list-title'; controls.append(since, kind, search); root.replaceChildren(comparison, runtime, listTitle, controls, hint, list, status);
  let after = true;
  const t = (key: keyof typeof copy.en) => copy[locale][key];
  const ct = (key: keyof typeof comparisonCopy.en) => comparisonCopy[locale][key];
  const date = (value: number) => new Date(value).toLocaleString(locale === 'en' ? 'en-US' : 'zh-CN');
  function button(label: string, target: RecoveryTarget) { const element = node('button', label); element.disabled = opening || loading || snapshot?.launchState !== 'idle'; element.onclick = () => void open(target); return element; }
  function format(field: ComparisonField, value: string | number | null) {
    if (value === null) return ct('none');
    if (field === 'javaMode') return t(value as 'inherit' | 'automatic' | 'custom');
    if (field === 'javaMajor') return `Java ${value}`;
    if (field === 'memoryMb') return `${value} MiB`;
    return String(value);
  }
  function renderComparison() {
    comparison.replaceChildren(); comparison.hidden = false;
    if (!snapshot) return;
    const value = snapshot.comparison;
    comparison.append(node('h3', ct('title')));
    if (!value.available) { comparison.append(node('p', ct(value.reason ?? 'noFailedAttempt'))); return; }
    if (value.baselineEndedAt !== null && value.failureAt !== null) {
      const dates = node('p', `${date(value.baselineEndedAt)} → ${date(value.failureAt)}`); dates.className = 'recovery-comparison-dates'; comparison.append(dates);
    }
    const changes = comparisonChanges(value), records = comparisonRecords(value);
    if (changes.length) {
      const heading = node('h4', ct('settings')), facts = node('dl'); facts.className = 'recovery-comparison-facts';
      for (const change of changes) { const row = node('div'); row.append(node('dt', ct(change.field)), node('dd', `${format(change.field, change.before)} → ${format(change.field, change.after)}`)); facts.append(row); }
      comparison.append(heading, facts);
    }
    if (records.length) {
      const heading = node('h4', ct('changes')), items = node('ol'); items.className = 'recovery-comparison-records';
      for (const record of records) {
        const item = node('li'), title = node('div'), name = node('strong', record.name), at = node('time', date(record.at)); at.dateTime = new Date(record.at).toISOString(); title.append(name, at); item.append(title);
        item.append(node('p', record.kind === 'config' ? ct('backup') : ct('modStarted')));
        if (record.detail.length) { const details = node('ul'); for (const detail of record.detail) details.append(node('li', detail)); item.append(details); }
        if (record.target) item.append(button(t(record.kind === 'mod' ? 'modOpen' : 'configOpen'), record.target)); items.append(item);
      }
      comparison.append(heading, items);
    }
    if (!changes.length && !records.length && value.runtime.changed === false && value.loader.changed === false) comparison.append(node('p', ct('noChanges')));
    if (value.runtime.changed === null || value.loader.changed === null) comparison.append(node('p', ct('unknown')));
    if (value.limited) comparison.append(node('p', ct('limited')));
    const gap = node('p', ct('external')); gap.className = 'recovery-comparison-gap'; comparison.append(gap);
  }
  function renderList() {
    list.replaceChildren(); if (!snapshot) return;
    const result = recoveryEvents(snapshot, after, search.value, kind.value as typeof modes[number], after && snapshot.comparison.available ? snapshot.comparison.failureAt : null);
    for (const item of result.items) {
      const row = node('li'), header = node('div'), title = node('strong', item.name), at = node('time', date(item.at)); at.dateTime = new Date(item.at).toISOString(); header.append(title, at);
      const details = node('p', [item.kind === 'mod' ? t('mods') : t('configs'), t(item.status in copy.en ? item.status as keyof typeof copy.en : 'unknownStatus')].join(' · ')); row.append(header, details);
      if (item.detail.length) { const changes = node('ul'); for (const text of item.detail) changes.append(node('li', text)); row.append(changes); }
      if (item.target) row.append(button(t(item.kind === 'mod' ? 'modOpen' : 'configOpen'), item.target)); list.append(row);
    }
    if (!result.items.length) list.append(node('li', t('empty')));
    hint.textContent = [result.limited ? t('limited') : '', !snapshot.mods.available ? t('missingMods') : '', !snapshot.configs.available ? t('missingConfigs') : ''].filter(Boolean).join(' ');
  }
  function render() {
    root.setAttribute('aria-busy', String(loading || opening));
    runtime.replaceChildren(); renderComparison(); controls.hidden = !snapshot; list.hidden = !snapshot; hint.hidden = !snapshot;
    runtime.hidden = !snapshot;
    listTitle.hidden = !snapshot; listTitle.textContent = after && snapshot?.comparison.available ? ct('later') : locale === 'zh-CN' ? '记录的变更' : 'Recorded changes';
    search.placeholder = t('search'); search.setAttribute('aria-label', t('search')); kind.setAttribute('aria-label', t('kinds'));
    [...kind.options].forEach((option, index) => option.textContent = t((['every', 'mods', 'configs'] as const)[index]));
    since.textContent = snapshot?.comparison.available ? ct(after ? 'allHistory' : 'laterOnly') : after && snapshot?.lastCleanLaunch ? t('since') : t('all'); since.setAttribute('aria-pressed', String(after && !!snapshot?.lastCleanLaunch)); since.hidden = !!snapshot && !snapshot.lastCleanLaunch;
    if (!snapshot) { if (loading || !gameId) comparison.append(node('p', loading ? t('loading') : t('noGame'))); return; }
    const current = snapshot.runtime; runtime.append(node('h3', ct('current')));
    const facts = node('dl'); facts.className = 'recovery-runtime-facts';
    for (const [label, value] of [[t('java'), `${t(current.javaMode)} · ${current.selectedJavaMajor ? `Java ${current.selectedJavaMajor}` : t('unknownJava')}`], [t('memory'), `${current.memoryMb} MiB`], [t('arguments'), String(current.jvmArgumentCount)]]) { const fact = node('div'); fact.append(node('dt', label), node('dd', value)); facts.append(fact); }
    runtime.append(facts);
    runtime.append(button(t('runtimeOpen'), { kind: 'runtimeSettings' }));
    if (snapshot.launchState !== 'idle') runtime.append(node('p', t('running')));
    renderList();
  }
  async function load() {
    if (!active || !gameId || opening) return; const version = ++request, selected = gameId; loading = true; status.textContent = ''; render();
    try { const value = await api.snapshot(selected); if (version !== request || selected !== gameId || !active) return; snapshot = value; }
    catch { if (version === request) { snapshot = null; status.textContent = t('unavailable'); } }
    finally { if (version === request) { loading = false; render(); } }
  }
  async function open(target: RecoveryTarget) {
    if (!snapshot || opening || loading || snapshot.launchState !== 'idle') return;
    const version = request; opening = true; status.textContent = t('opening'); render();
    try { await api.open(gameId, snapshot.revision, target); if (version === request && active) status.textContent = t('opened'); }
    catch (cause) { if (version === request) status.textContent = String(cause).includes('RECOVERY_SNAPSHOT_CHANGED') ? t('changedError') : t('failed'); }
    finally { if (version === request) { opening = false; render(); } }
  }
  search.oninput = () => renderList(); kind.onchange = () => renderList(); since.onclick = () => { after = !after; render(); };
  return { refresh: load, update(id: string, language: Locale, shown: boolean) {
    locale = language; const changed = id !== gameId, entered = shown && !active;
    if (changed || !shown && active) { ++request; loading = false; opening = false; status.textContent = ''; }
    if (changed) { gameId = id; snapshot = null; search.value = ''; kind.value = 'all'; after = true; }
    active = shown; render(); if (shown && (changed || entered)) void load();
  }, dispose() { ++request; active = false; } };
}
