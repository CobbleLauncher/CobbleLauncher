// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext, connect, type Client, type Report } from '../../../../packages/plugin-web-sdk/src/index';
import type { LanguageSelection } from '../../../../packages/plugin-web-sdk/src/lang-package';
import { defaultLocales, issuesOf, KINDS, kindName, localeChoices, metadataOf, reportText, sourceLines, visibleIssues, type Issue } from './model';
import { button, createView, el, option } from './view';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
export function mountLanguageDoctor(client: Client, host = document.getElementById('app')!) {
    let zh = false, busy = false, checking = false, epoch = 0, detailEpoch = 0;
    let selection: LanguageSelection | null = null, report: Report | null = null;
    let issues: Issue[] = [], selected: Issue | null = null;
    let view: ReturnType<typeof createView>;
    const tr = (en: string, cn: string) => zh ? cn : en;
    const namespace = () => view.namespace.value === '__loose__' ? null : view.namespace.value;
    function fail(cause: unknown) {
        const code = String(cause);
        view.error.hidden = false;
        view.error.textContent = /LIMIT|LARGE/.test(code) ? tr('These files exceed the checker’s limits. Choose a smaller resource pack.', '文件超出检查范围，请选择更小的资源包。')
            : /NO_LANGUAGE/.test(code) ? tr('No language files were found in this selection.', '所选内容中未找到语言文件。')
            : /PERMISSION|REVOKED/.test(code) ? tr('Review this tool’s file permissions and try again.', '请检查此工具的文件权限后重试。')
            : /SELECTION/.test(code) ? tr('Choose the files again to continue.', '请重新选择文件后继续。')
            : tr('Could not complete the check. Your original files are unchanged.', '检查未完成，原文件未更改。');
    }
    function controls() {
        host.setAttribute('aria-busy', String(busy));
        view.metrics.hidden = !report;
        view.pick.disabled = busy; view.download.disabled = busy || !report;
        for (const node of [view.namespace, view.baseline, view.target]) node.disabled = busy || !selection;
        view.check.disabled = busy || !selection || !view.target.value || (!!view.baseline.value && view.baseline.value === view.target.value);
        view.cancel.hidden = !checking;
    }
    function detailReset() {
        detailEpoch++; selected = null; view.detailTitle.textContent = tr('Select an issue', '选择问题');
        view.message.textContent = ''; view.reference.textContent = ''; view.translation.textContent = ''; view.values.hidden = true;
        view.fileName.textContent = ''; view.code.replaceChildren();
    }
    function resetReport() {
        report = null; issues = []; view.body.replaceChildren();
        for (const node of [view.referenceCount, view.targetCount, view.issueCount]) node.textContent = '—';
        view.notice.hidden = true; detailReset(); rows(); controls();
    }
    function localeOptions() {
        view.baseline.replaceChildren(); view.target.replaceChildren(); option(view.baseline, '', tr('Syntax only', '仅检查格式'));
        const locales = selection ? localeChoices(selection, namespace()) : [];
        for (const locale of locales) { option(view.baseline, locale, locale); option(view.target, locale, locale); }
        const defaults = defaultLocales(locales); view.baseline.value = defaults.baseline; view.target.value = defaults.target;
        resetReport();
    }
    async function showIssue(issue: Issue) {
        if (!selection) return;
        const current = ++detailEpoch; selected = issue;
        view.detailTitle.textContent = issue.key || kindName(issue.kind, zh); view.message.textContent = issue.message;
        view.values.hidden = false; view.reference.textContent = issue.reference || '—'; view.translation.textContent = issue.translation || '—';
        view.code.replaceChildren(); view.fileName.textContent = tr('Loading source…', '正在读取原文…'); rows();
        if (!selection.files.some(file => file.id === issue.file)) { view.fileName.textContent = tr('Source unavailable.', '原文不可读取。'); return; }
        try {
            const document = await client.langPackage.document({ selectionId: selection.selectionId, fileId: issue.file });
            if (current !== detailEpoch) return;
            view.fileName.textContent = `${document.name}${issue.line ? `:${issue.line}` : ''}`;
            for (const line of sourceLines(document.text, issue.line)) {
                const node = el('span', '', 'source-line'); node.dataset.active = String(line.active);
                node.append(el('span', String(line.number), 'line-number'), el('span', line.text)); view.code.append(node);
            }
        } catch (cause) { if (current === detailEpoch) { view.fileName.textContent = tr('Source unavailable.', '原文不可读取。'); fail(cause); } }
    }
    function rows() {
        if (!view) return;
        const matches = visibleIssues(issues, view.search.value, view.filter.value);
        const fragment = document.createDocumentFragment();
        for (const issue of matches) {
            const row = el('tr'); row.dataset.active = String(issue === selected);
            const key = el('td'), choose = button(issue.key || tr('File', '文件'), 'select-issue', () => void showIssue(issue)); choose.className = 'issue-key'; key.append(choose);
            row.append(key, el('td', kindName(issue.kind, zh)), el('td', issue.line?.toString() ?? '—')); fragment.append(row);
        }
        view.body.replaceChildren(fragment); view.empty.hidden = matches.length > 0;
        view.empty.textContent = !selection ? tr('Choose a resource pack ZIP or language JSON files.', '选择资源包 ZIP 或语言 JSON 文件。')
            : !report ? tr('Choose languages, then run a check.', '选择语言后开始检查。')
            : issues.length ? tr('No matching issues.', '没有匹配的问题。') : tr('No issues found.', '未发现问题。');
    }
    async function run(action: (current: number) => Promise<void>, check = false) {
        if (busy) return;
        busy = true; checking = check; const current = ++epoch; view.error.hidden = true; view.status.textContent = ''; controls();
        try { await action(current); } catch (cause) { if (current === epoch) fail(cause); }
        finally { if (current === epoch) { busy = false; checking = false; controls(); } }
    }
    async function start() {
        const context = await hostContext(client.context()); zh = context.locale === 'zh-CN'; document.documentElement.lang = context.locale; document.documentElement.dataset.theme = context.theme;
        host.className = 'workbench language-doctor'; view = createView(host, tr);
        option(view.filter, '', tr('All issues', '全部问题')); for (const kind of KINDS) option(view.filter, kind, kindName(kind, zh));
        view.pick.onclick = () => void run(async current => {
            const picked = await client.langPackage.pick(); if (current !== epoch || !picked) return;
            selection = picked; view.source.textContent = picked.sourceName; view.format.textContent = picked.packFormat?.toString() ?? '—';
            view.namespace.replaceChildren();
            for (const name of [...new Set(picked.files.map(file => file.namespace))]) option(view.namespace, name ?? '__loose__', name ?? tr('Language files', '语言文件'));
            localeOptions();
        });
        view.namespace.onchange = localeOptions;
        for (const node of [view.baseline, view.target]) node.onchange = () => { resetReport(); controls(); };
        view.search.oninput = rows; view.filter.onchange = rows;
        view.check.onclick = () => void run(async current => {
            if (!selection || !view.target.value || (view.baseline.value && view.baseline.value === view.target.value)) return;
            const result = await client.langPackage.check({ selectionId: selection.selectionId, namespace: namespace(), baselineLocale: view.baseline.value || null, targetLocale: view.target.value });
            if (current !== epoch || !result) return;
            report = result; issues = issuesOf(result); detailReset();
            const metadata = metadataOf(result); view.referenceCount.textContent = metadata.reference_keys || '—'; view.targetCount.textContent = metadata.target_keys || '—'; view.issueCount.textContent = metadata.issue_count || String(issues.length);
            const notices = result.findings.filter(finding => /limit|truncat/i.test(finding.code)); view.notice.textContent = notices.map(finding => finding.message).join(' '); view.notice.hidden = !notices.length;
            rows(); view.status.textContent = tr('Check complete.', '检查完成。');
        }, true);
        view.cancel.onclick = async () => {
            epoch++; detailEpoch++;
            try { await client.langPackage.cancel(); selection = null; view.source.textContent = ''; view.namespace.replaceChildren(); view.baseline.replaceChildren(); view.target.replaceChildren(); resetReport(); view.status.textContent = tr('Check cancelled. Choose files to start again.', '检查已取消，请重新选择文件。'); }
            catch (cause) { fail(cause); }
            finally { busy = false; checking = false; controls(); }
        };
        view.download.onclick = () => void run(async () => {
            if (!report || !selection) return;
            const result = await client.exports.text({ name: 'language-report.txt', text: reportText(report, selection.sourceName) });
            view.status.textContent = result.saved ? tr('Report exported.', '报告已导出。') : '';
        });
        resetReport(); controls();
    }
    void start().catch(cause => { if (view) fail(cause); else host.textContent = 'Could not open Language Doctor / 无法打开语言检查'; });
}
try { mountLanguageDoctor(connect()); }
catch { document.getElementById('app')!.textContent = 'Open Language Doctor from Cobble Launcher / 请从 Cobble Launcher 打开语言检查'; }

installPluginPresentation(presentation);
