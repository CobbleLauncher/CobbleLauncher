// SPDX-License-Identifier: MIT
import { redactLog, type LogRedaction, type RedactionKind } from '../../../../packages/plugin-web-sdk/src/redaction';
import type { DiagnosticDocument } from '../../../../packages/plugin-web-sdk/src/index';
import { logPreview, sharingLimits, type LogView } from './redaction-model';
const words = {
  en: { redacted: 'Redacted', changes: 'Changes', loaded: 'Loaded text', search: 'Search this view', copy: 'Copy redacted text', export: 'Export redacted log', review: 'Review before sharing. Patterns may miss private information.', original: 'Copy and export always use the redacted text.', empty: 'Choose a game log to redact.', noMatches: 'No matching lines.', unchanged: 'No additional matches found.', limited: 'Preview limited. Export includes the full redacted log.', copyLimit: 'Clipboard limit: 8 KiB. Export the full log.', exportLimit: 'Redacted output exceeds the 1 MiB export limit.', copied: 'Copied redacted text.', saved: 'Redacted log exported.', failed: 'Could not finish. Try again.', working: 'Working…', credential: 'Credentials', account: 'Accounts', path: 'Paths', address: 'Addresses', count: 'redactions', lines: 'Log preview' },
  'zh-CN': { redacted: '脱敏文本', changes: '变更行', loaded: '读取文本', search: '搜索当前视图', copy: '复制脱敏文本', export: '导出脱敏日志', review: '分享前请检查，自动规则可能遗漏隐私信息。', original: '复制和导出始终使用脱敏文本。', empty: '选择游戏日志以脱敏。', noMatches: '没有匹配的行。', unchanged: '未发现其他匹配内容。', limited: '预览已限量，导出包含完整脱敏日志。', copyLimit: '剪贴板上限为 8 KiB，请导出完整日志。', exportLimit: '脱敏结果超过 1 MiB 导出上限。', copied: '已复制脱敏文本。', saved: '已导出脱敏日志。', failed: '未能完成，请重试。', working: '正在处理…', credential: '凭据', account: '账号', path: '路径', address: '地址', count: '处脱敏', lines: '日志预览' },
};
type Locale = keyof typeof words;
type Actions = { copy: (text: string) => Promise<unknown>; export: (text: string) => Promise<{ saved: boolean }> };
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text = '') { const node = document.createElement(tag); node.textContent = text; return node; }
export function createRedactionView(root: HTMLElement, actions: Actions) {
  let source: DiagnosticDocument | null = null, result: LogRedaction | null = null, locale: Locale = 'en', mode: LogView = 'redacted', generation = 0, operating = false, blocked = false;
  const t = (key: keyof typeof words.en) => words[locale][key];
  const note = element('p'), tools = element('div'), modes = element('div'), search = element('input'), output = element('div'), counts = element('p'), status = element('p'), limit = element('p');
  const copy = element('button'), save = element('button'), buttons = new Map<LogView, HTMLButtonElement>();
  note.className = 'redaction-note'; tools.className = 'redaction-tools'; modes.className = 'redaction-modes'; counts.className = 'redaction-counts'; limit.className = 'redaction-limit'; status.className = 'redaction-status'; output.className = 'redaction-output';
  search.type = 'search'; search.maxLength = 120; output.tabIndex = 0; status.setAttribute('role', 'status');
  for (const value of ['redacted', 'changes', 'loaded'] as const) {
    const button = element('button'); buttons.set(value, button); button.onclick = () => { mode = value; status.textContent = ''; render(); }; modes.append(button);
  }
  const controls = element('div'); controls.className = 'redaction-controls'; controls.append(modes, search); tools.append(copy, save);
  root.replaceChildren(note, counts, controls, output, limit, tools, status);
  function preview() {
    output.replaceChildren();
    if (!source || !result) { output.append(element('p', t('empty'))); return; }
    const preview = logPreview(source.text, result, mode, search.value);
    if (!preview.lines.length) output.append(element('p', mode === 'changes' && !search.value ? t('unchanged') : t('noMatches')));
    for (const line of preview.lines) {
      const row = element('div'), number = element('span', String(line.number)), code = element('code');
      number.className = 'redaction-line-number'; number.setAttribute('aria-hidden', 'true'); row.className = 'redaction-line';
      let cursor = 0;
      for (const mark of line.marks) { code.append(document.createTextNode(line.text.slice(cursor, mark.start)), element('mark', line.text.slice(mark.start, mark.end))); cursor = mark.end; }
      code.append(document.createTextNode(line.text.slice(cursor) || (line.text ? '' : ' '))); row.append(number, code); output.append(row);
    }
    limit.textContent = preview.limited ? t('limited') : '';
  }
  function render() {
    note.textContent = t('review') + (mode === 'loaded' ? ` ${t('original')}` : '');
    for (const [value, button] of buttons) { button.textContent = t(value); button.setAttribute('aria-pressed', String(mode === value)); button.disabled = !source; }
    search.placeholder = t('search'); search.setAttribute('aria-label', t('search')); search.disabled = !source; output.setAttribute('aria-label', t('lines'));
    copy.textContent = t('copy'); save.textContent = t('export'); const limits = sharingLimits(result?.text ?? '');
    copy.disabled = blocked || operating || !limits.copy; save.disabled = blocked || operating || !limits.export;
    counts.textContent = result ? `${result.totalChanges} ${t('count')} · ${Object.entries(result.counts).filter(([, count]) => count).map(([kind, count]) => `${t(kind as RedactionKind)} ${count}`).join(' · ')}`.replace(/ · $/u, '') : '';
    preview();
    if (source && result && !limits.copy && limits.export) limit.textContent = [limit.textContent, t('copyLimit')].filter(Boolean).join(' ');
    if (source && result && !limits.export && result.text) limit.textContent = [limit.textContent, t('exportLimit')].filter(Boolean).join(' ');
  }
  async function share(kind: 'copy' | 'export') {
    const value = result, revision = generation; if (!value || operating || blocked || !sharingLimits(value.text)[kind]) return;
    operating = true; status.textContent = t('working'); render();
    try { const saved = kind === 'copy' ? (await actions.copy(value.text), true) : (await actions.export(value.text)).saved; if (revision === generation) status.textContent = saved ? t(kind === 'copy' ? 'copied' : 'saved') : ''; }
    catch { if (revision === generation) status.textContent = t('failed'); }
    finally { if (revision === generation) { operating = false; render(); } }
  }
  copy.onclick = () => void share('copy'); save.onclick = () => void share('export'); search.oninput = () => render();
  return { update(value: DiagnosticDocument | null, language: Locale, disabled: boolean) {
    locale = language; blocked = disabled;
    if (value !== source) {
      generation++; source = value; result = null; operating = false; mode = 'redacted'; search.value = ''; status.textContent = ''; limit.textContent = '';
      if (value) { try { result = redactLog(value.text); } catch { status.textContent = t('failed'); } }
    }
    render();
  } };
}
