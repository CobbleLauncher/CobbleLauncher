// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext, connect, parseFileInboxLink, type Client, type FileInboxItem, type FileInboxLink } from '../../../../packages/plugin-web-sdk/src/index';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';

export async function mountInbox(client: Client, host = document.getElementById('app')!) {
  const context = await hostContext(client.context());
  document.documentElement.lang = context.locale;
  document.documentElement.dataset.theme = context.theme;
  const zh = context.locale === 'zh-CN';
  const tr = (en: string, cn: string) => zh ? cn : en;
  const label = (v: string) => ({
    mrpack: tr('Modrinth pack', 'Modrinth 整合包'), 'curseforge-pack': tr('CurseForge pack', 'CurseForge 整合包'),
    mod: 'Mod', modpack: tr('Modpack', '整合包'), resource_pack: tr('Resource pack', '资源包'),
    shader_pack: tr('Shader pack', '光影包'), 'resource-pack': tr('Resource pack', '资源包'),
    'shader-pack': tr('Shader pack', '光影包'), blueprint: tr('Blueprint', '蓝图'),
    log: tr('Log', '日志'), ambiguous: tr('Choose a format', '请选择格式'),
    unknown: tr('Unknown format', '未知格式'),
  } as Record<string, string>)[v] ?? v;
  const size = (n: number) => n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(1)} MB`;
  const el = <K extends keyof HTMLElementTagNameMap>(tag: K, content = '', cls = '') => {
    const node = document.createElement(tag); node.textContent = content; node.className = cls; return node;
  };
  const button = (content: string, action: () => void, cls = '') => {
    const node = el('button', content, cls); node.type = 'button'; node.onclick = action; return node;
  };
  let items: FileInboxItem[] = [], active = '', handler = '', game = '', generation = 0;
  let busy = false, message = '', failure = '', url = '', linkGame = context.instanceId ?? '';
  function recommendation(item: FileInboxItem) {
    handler = item.kind === 'ambiguous' ? '' : item.handlers.length === 1 ? item.handlers[0].id : '';
    game = context.instanceId && item.games.some(g => g.id === context.instanceId && g.available) ? context.instanceId : '';
  }
  function select(id: string) {
    active = id; const item = items.find(value => value.resourceId === id);
    if (item) recommendation(item); else { handler = ''; game = ''; }
    message = ''; failure = ''; render();
  }
  function cancel() { generation++; busy = false; message = tr('Cancelled.', '已取消。'); render(); }
  function error(value: unknown) {
    const code = String(value);
    return /LINK_UNSUPPORTED/.test(code) ? tr('Use a Modrinth or CurseForge project or version page.', '请粘贴 Modrinth 或 CurseForge 的项目或版本页面链接。')
      : /GAME_REQUIRED/.test(code) ? tr('Choose a game first.', '请先选择游戏。')
      : /PERMISSION/.test(code) ? tr('Allow file import requests in this extension’s permissions.', '请在扩展权限中允许文件导入请求。')
      : /LIMIT/.test(code) ? tr('Choose up to 8 files, at most 512 MB each and 1 GB total.', '最多选择 8 个文件，每个不超过 512 MB，总计不超过 1 GB。')
      : /EXPIRED/.test(code) ? tr('This file expired. Choose it again.', '文件已过期，请重新选择。')
      : /ACTION_PENDING/.test(code) ? tr('Complete the open review first.', '请先完成已打开的确认。')
      : tr('Could not complete this request. Try again.', '操作未完成，请重试。');
  }
  async function load(pick = false) {
    const token = ++generation; busy = true; failure = ''; message = ''; render();
    try {
      const next = await (pick ? client.fileInbox.pick() : client.fileInbox.pending());
      if (token !== generation) return;
      items = next;
      if (!items.some(item => item.resourceId === active)) {
        active = items[0]?.resourceId ?? '';
        if (items[0]) recommendation(items[0]); else { handler = ''; game = ''; }
      }
    } catch (cause) { if (token === generation) failure = error(cause); }
    finally { if (token === generation) { busy = false; render(); } }
  }
  async function release(id: string) {
    const token = ++generation; busy = true; render();
    try {
      await client.fileInbox.release({ resourceIds: [id] });
      if (token !== generation) return;
      items = items.filter(item => item.resourceId !== id);
      if (active === id) { active = items[0]?.resourceId ?? ''; if (items[0]) recommendation(items[0]); }
    } catch (cause) { if (token === generation) failure = error(cause); }
    finally { if (token === generation) { busy = false; render(); } }
  }
  async function openFile() {
    const item = items.find(value => value.resourceId === active);
    const chosen = item?.handlers.find(value => value.id === handler);
    if (!item || !chosen || chosen.requiresGame && !game) return;
    const token = ++generation; busy = true; failure = ''; render();
    try {
      await client.fileInbox.open({ resourceId: active, handlerId: handler, ...(chosen.requiresGame ? { gameId: game } : {}) });
      if (token === generation) message = tr('Review requested in the launcher.', '已请求在启动器中查看。');
    } catch (cause) { if (token === generation) failure = error(cause); }
    finally { if (token === generation) { busy = false; render(); } }
  }
  function parsedLink(): FileInboxLink | null {
    try { return parseFileInboxLink(url.trim()); } catch { return null; }
  }
  async function openLink() {
    const link = parsedLink();
    if (!link || link.kind !== 'modpack' && !linkGame) { failure = error(link ? 'FILE_INBOX_GAME_REQUIRED' : 'FILE_INBOX_LINK_UNSUPPORTED'); render(); return; }
    const token = ++generation; busy = true; failure = ''; message = ''; render();
    try {
      await client.fileInbox.openLink({ url: url.trim(), ...(linkGame ? { gameId: linkGame } : {}) });
      if (token === generation) message = tr('Opened in Market. Review the version before installing.', '已在市场打开，请确认版本后安装。');
    } catch (cause) { if (token === generation) failure = error(cause); }
    finally { if (token === generation) { busy = false; render(); } }
  }
  function gameSelect(value: string, update: (id: string) => void, games: { id: string; name: string; version: string; available?: boolean; compatibility?: string }[]) {
    const select = el('select'); select.setAttribute('aria-label', tr('Target game', '目标游戏'));
    const placeholder = el('option', tr('Choose a game', '选择游戏')); placeholder.value = ''; select.append(placeholder);
    for (const game of games) {
      const option = el('option', `${game.name} · ${game.version}${game.available === false ? tr(' · Running', ' · 运行中') : game.compatibility === 'mismatch' ? tr(' · Version differs', ' · 版本不同') : ''}`);
      option.value = game.id; option.disabled = game.available === false; select.append(option);
    }
    select.value = value; select.onchange = () => update(select.value);
    return select;
  }
  function render() {
    host.replaceChildren(); host.className = 'workbench file-inbox'; host.dataset.theme = context.theme;
    const header = el('header', '', 'toolbar'); header.append(el('h1', tr('File Inbox', '文件投递')));
    const actions = el('div', '', 'actions'); actions.append(button(tr('Choose files', '选择文件'), () => void load(true), 'primary'));
    if (busy) actions.append(button(tr('Cancel', '取消'), cancel)); else actions.append(button(tr('Refresh', '刷新'), () => void load()));
    header.append(actions); host.append(header);
    const linkPanel = el('form', '', 'link-panel');
    const linkLabel = el('label', tr('Project or version link', '项目或版本链接'));
    const linkInput = el('input') as HTMLInputElement; linkInput.type = 'url'; linkInput.value = url;
    linkInput.placeholder = 'https://modrinth.com/mod/…'; linkInput.setAttribute('aria-label', tr('Project or version link', '项目或版本链接'));
    linkInput.oninput = () => { url = linkInput.value; updateLink(); };
    linkLabel.append(linkInput); linkPanel.append(linkLabel);
    const linkDetails = el('div', '', 'link-details'); linkPanel.append(linkDetails);
    const linkAction = button(tr('Open in Market', '在市场打开'), () => void openLink(), 'primary link-open'); linkPanel.append(linkAction);
    linkPanel.onsubmit = event => { event.preventDefault(); void openLink(); };
    host.append(linkPanel);
    function updateLink() {
      const link = parsedLink(); linkDetails.replaceChildren();
      if (url.trim()) {
        if (!link) linkDetails.append(el('p', tr('Use a Modrinth or CurseForge project or version page. Links to files are unsupported.', '请使用 Modrinth 或 CurseForge 的项目或版本页面。文件直链不受支持。'), 'muted'));
        else {
          linkDetails.append(el('p', `${link.provider === 'modrinth' ? 'Modrinth' : 'CurseForge'} · ${label(link.kind)} · ${link.project}${link.version ? ` · ${link.version}` : ''}`, 'metadata'));
          if (link.kind !== 'modpack') linkDetails.append(gameSelect(linkGame, id => { linkGame = id; updateLink(); }, context.games));
        }
      }
      linkAction.disabled = busy || !link || link.kind !== 'modpack' && !linkGame;
    }
    updateLink();
    if (message || busy) { const status = el('p', busy ? tr('Loading…', '正在加载…') : message, 'status'); status.role = 'status'; host.append(status); }
    if (failure) { const warning = el('p', failure, 'error'); warning.role = 'alert'; host.append(warning); }
    if (!items.length) {
      const empty = el('section', '', 'empty-inbox');
      empty.append(el('div', '↓', 'drop-symbol'), el('p', tr('Drop a file here or choose files.', '拖入文件，或选择文件。')));
      host.append(empty); return;
    }
    const layout = el('div', '', 'inbox-layout'), queue = el('nav', '', 'queue'), detail = el('section', '', 'inbox-detail');
    queue.setAttribute('aria-label', tr('Selected files', '所选文件'));
    queue.append(el('h2', tr(`Files (${items.length})`, `文件 (${items.length})`)));
    for (const item of items) {
      const row = button('', () => select(item.resourceId), 'file-row'); row.setAttribute('aria-current', String(item.resourceId === active));
      row.append(el('strong', item.name), el('span', `${label(item.kind)} · ${size(item.bytes)}`)); queue.append(row);
    }
    const item = items.find(value => value.resourceId === active);
    if (item) {
      const top = el('div', '', 'detail-heading'); top.append(el('h2', item.name), button(tr('Remove', '移除'), () => void release(item.resourceId)));
      detail.append(top, el('p', `${label(item.kind)} · ${size(item.bytes)}`, 'muted'));
      if (item.declaredMinecraft || item.declaredLoader) detail.append(el('p', [item.declaredMinecraft ? `Minecraft ${item.declaredMinecraft}` : '', item.declaredLoader ?? ''].filter(Boolean).join(' · '), 'metadata'));
      if (item.kind === 'ambiguous') detail.append(el('p', tr('Several formats were found. Choose how to open this file.', '发现多种格式，请选择打开方式。'), 'notice'));
      if (item.error) detail.append(el('p', tr('The file could not be inspected.', '无法检查此文件。'), 'error'));
      if (!item.handlers.length) detail.append(el('p', tr('No enabled handler supports this file.', '没有已启用的处理器支持此文件。'), 'notice'));
      else {
        const field = el('fieldset'), legend = el('legend', tr('Open with', '打开方式')); field.append(legend);
        for (const option of item.handlers) {
          const row = el('label', '', 'handler'), radio = el('input'); radio.type = 'radio'; radio.name = 'handler'; radio.value = option.id;
          radio.checked = handler === option.id; radio.onchange = () => { handler = option.id; render(); };
          row.append(radio, el('span', option.label)); field.append(row);
        }
        detail.append(field);
        const chosen = item.handlers.find(value => value.id === handler);
        if (chosen?.requiresGame) {
          const field = el('label', tr('Target game', '目标游戏'), 'game-label');
          field.append(gameSelect(game, id => { game = id; render(); }, item.games)); detail.append(field);
          const target = item.games.find(value => value.id === game);
          if (target) detail.append(el('p', target.compatibility === 'declared-match'
            ? tr('Declared Minecraft version matches.', '声明的 Minecraft 版本相同。')
            : target.compatibility === 'mismatch'
              ? tr('Check the file’s required version before importing.', '导入前请检查文件要求的版本。')
              : tr('Compatibility is not declared. Check the file’s requirements.', '兼容性未声明，请检查文件要求。'), 'muted'));
        }
        const submit = button(tr('Continue', '继续'), () => void openFile(), 'primary review');
        submit.disabled = busy || !chosen || chosen.requiresGame && !game; detail.append(submit);
      }
    }
    layout.append(queue, detail); host.append(layout);
    for (const control of host.querySelectorAll('button')) if (control.textContent !== tr('Cancel', '取消') && busy) control.disabled = true;
  }
  host.addEventListener('keydown', event => { if (event.key === 'Escape' && busy) cancel(); });
  render(); await load();
  return { cancel, refresh: () => load(), destroy: () => { generation++; host.replaceChildren(); } };
}
try { await mountInbox(connect()); }
catch { const host = document.getElementById('app'); if (host) host.textContent = 'Open File Inbox from the launcher. / 请从启动器打开文件投递。'; }
installPluginPresentation(presentation);
