// SPDX-License-Identifier: MIT
/** Local pattern-based log cleanup. Review the result before sharing. */
export type RedactionKind = 'credential' | 'account' | 'path' | 'address';
export type RedactionChange = { start: number; end: number; line: number; kind: RedactionKind };
export type LogRedaction = {
  text: string; changes: RedactionChange[]; counts: Record<RedactionKind, number>;
  totalChanges: number; changesTruncated: boolean;
};
type Range = { start: number; end: number; kind: RedactionKind };
const labels: Record<RedactionKind, string> = { credential: '[CREDENTIAL]', account: '[ACCOUNT]', path: '[PATH]', address: '[ADDRESS]' };
const scalar = String.raw`(?:\[(?:CREDENTIAL|ACCOUNT|PATH|ADDRESS|REDACTED)\]|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s,;}\]&]+)`;
const secretKey = String.raw`(?:access[_-]?token|refresh[_-]?token|client[_-]?secret|password|passwd|session[_-]?(?:id|token)|api[_-]?key|x-api-key)`;
const accountKey = String.raw`(?:user[_-]?(?:name|id)|profile[_-]?(?:id|name)|account[_-]?(?:id|name)|xuid|uuid)`;
function lineRanges(line: string): Range[] {
  const ranges: Range[] = [];
  const add = (start: number, end: number, kind: RedactionKind) => { if (end > start) ranges.push({ start, end, kind }); };
  const scan = (pattern: RegExp, kind: RedactionKind, valueGroup = 0) => {
    for (const match of line.matchAll(pattern)) {
      const value = match[valueGroup]!;
      if (/^\[(?:CREDENTIAL|ACCOUNT|PATH|ADDRESS|REDACTED)\]$/u.test(value)) continue;
      let start = match.index! + (valueGroup ? match[0].lastIndexOf(value) : 0), end = start + value.length;
      if (valueGroup && /^(["']).*\1$/u.test(value)) { start++; end--; }
      if (kind === 'account' && ranges.some(range => range.kind === 'credential' && start >= range.start && start < range.end)) continue;
      add(start, end, kind);
    }
  };
  // Headers may carry spaces (Bearer/Basic, cookies); quoted JSON stops at its own quote.
  scan(/\b(?:authorization|proxy-authorization|cookie|set-cookie)["']?\s*[:=]\s*("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\r\n]+)/giu, 'credential', 1);
  scan(new RegExp(String.raw`\b${secretKey}["']?\s*[:=]\s*(${scalar})`, 'giu'), 'credential', 1);
  scan(new RegExp(String.raw`--(?:accessToken|refreshToken|clientSecret|password|session|apiKey)(?:=|\s+)(${scalar})`, 'giu'), 'credential', 1);
  scan(/\bhttps?:\/\/([^\s/@]+)@/giu, 'credential', 1);
  scan(/\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/gu, 'credential');
  scan(new RegExp(String.raw`\b${accountKey}["']?\s*[:=]\s*(${scalar})`, 'giu'), 'account', 1);
  scan(new RegExp(String.raw`--(?:username|uuid|xuid|clientId)(?:=|\s+)(${scalar})`, 'giu'), 'account', 1);
  scan(/\bSetting user:\s*(\S+)/giu, 'account', 1);
  scan(/\bUUID of player\s+(\S+)\s+is\b/giu, 'account', 1);
  scan(/\b([A-Za-z0-9_]{1,16})(?=\[\/[^\]]+\]\s+logged in\b)/gu, 'account', 1);
  scan(/\b([A-Za-z0-9_]{1,16})\s+(?:joined the game|left the game|lost connection:)/gu, 'account', 1);
  scan(/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/giu, 'account');
  scan(/\b[A-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Z0-9](?:[A-Z0-9.-]*[A-Z0-9])?\.[A-Z]{2,}\b/giu, 'account');
  // Keep quoted paths containing spaces together. A relative JAR/class name is not a path.
  scan(/(["'])(?:[A-Z]:[\\/]|\\\\|\/)[^\r\n]*?\1/giu, 'path');
  scan(/\b[A-Z]:[\\/](?:[^\\/\r\n"'<>|,;\])}]+[\\/])*[^\s"'<>|,;\])}]+/giu, 'path');
  scan(/\\\\[^\s"'<>|,;\])}]+/gu, 'path');
  scan(/(?<![\w.:/])\/(?!\/)(?:[^/\r\n"'<>|,;\])}]+\/)+[^\s"'<>|,;\])}]+/gu, 'path');
  for (const match of line.matchAll(/(?<![\w.])(?:\d{1,3}\.){3}\d{1,3}(?![\w.])/gu)) {
    const before = line.slice(Math.max(0, match.index! - 48), match.index!);
    if (/(?:version|minecraft|forge|neoforge|fabric|loader)\s*[:=v]?\s*$/iu.test(before)) continue;
    if (match[0].split('.').every(part => Number(part) <= 255)) add(match.index!, match.index! + match[0].length, 'address');
  }
  for (const match of line.matchAll(/(?<![\w:])(?:[0-9a-f]{0,4}:){2,}[0-9a-f:.]*(?![\w:])/giu)) {
    const value = match[0];
    if (value.includes('::') || value.split(':').length >= 8) add(match.index!, match.index! + value.length, 'address');
  }
  ranges.sort((a, b) => a.start - b.start || b.end - a.end);
  const merged: Range[] = [];
  for (const range of ranges) {
    const last = merged.at(-1);
    if (last && range.start < last.end) {
      last.end = Math.max(last.end, range.end);
      if (range.kind === 'credential') last.kind = 'credential';
    } else merged.push({ ...range });
  }
  return merged;
}
/** Accepts at most 1 MiB UTF-8, retains CRLF/LF, and returns output-coordinate spans. */
export function redactLog(input: string): LogRedaction {
  if (typeof input !== 'string' || new TextEncoder().encode(input).length > 1_048_576) throw new Error('LOG_REDACTION_INPUT_INVALID');
  const result: LogRedaction = { text: '', changes: [], counts: { credential: 0, account: 0, path: 0, address: 0 }, totalChanges: 0, changesTruncated: false };
  let lineNumber = 0;
  for (const segment of input.split(/(?<=\n)/u)) {
    lineNumber++;
    const ending = segment.endsWith('\r\n') ? '\r\n' : segment.endsWith('\n') ? '\n' : '';
    const line = ending ? segment.slice(0, -ending.length) : segment;
    let cursor = 0;
    for (const range of lineRanges(line)) {
      result.text += line.slice(cursor, range.start);
      const start = result.text.length;
      result.text += labels[range.kind];
      if (result.changes.length < 2048) result.changes.push({ start, end: result.text.length, line: lineNumber, kind: range.kind });
      else result.changesTruncated = true;
      result.counts[range.kind]++; result.totalChanges++; cursor = range.end;
    }
    result.text += line.slice(cursor) + ending;
  }
  return result;
}
