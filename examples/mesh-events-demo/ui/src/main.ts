// SPDX-License-Identifier: MIT
import { connect, applyHostAppearance, type Client, type MeshNamespace, type MeshPeer, type MeshStatus } from '../../../../packages/plugin-web-sdk/src/index.ts';
import { readiness } from './ready.ts';
import '../../../../packages/plugin-workbench/src/style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
import './style.css';
const namespace = 'org.example.ready.v1';
function element<K extends keyof HTMLElementTagNameMap>(name: K, text = '', className = '') {
  const node = document.createElement(name); node.textContent = text; node.className = className; return node;
}
function errorCode(error: unknown) { return error instanceof Error ? error.message : String(error); }

/** Mounting starts reception only for this visible tool. Every send follows an explicit button press. */
export async function mountReadyCheck(client: Client, root = document.getElementById('app')!) {
  const context = await client.context(); applyHostAppearance(context);
  const zh = context.locale === 'zh-CN', text = (en: string, cn: string) => zh ? cn : en;
  root.replaceChildren();
  const heading = element('header', '', 'ready-heading');
  heading.append(element('h1', text('Ready check', '准备状态'))); root.append(heading);
  const layout = element('div', '', 'ready-layout'); root.append(layout);
  const form = element('form', '', 'ready-panel'), received = element('section', '', 'ready-panel ready-received'); layout.append(form, received);
  form.append(element('h2', text('Your status', '你的状态')));
  const label = element('label', text('Friend', '朋友')), select = element('select'); select.id = 'ready-peer'; label.htmlFor = select.id;
  const refresh = element('button', text('Refresh friends', '刷新朋友'), 'ready-refresh'); refresh.type = 'button';
  const peerTools = element('div', '', 'ready-peer-tools'); peerTools.append(select, refresh); form.append(label, peerTools);
  const peerStatus = element('p', '', 'status'); peerStatus.setAttribute('role', 'status'); form.append(peerStatus);
  const readyLabel = element('label', '', 'ready-checkbox'), ready = element('input'); ready.type = 'checkbox'; ready.id = 'ready-value';
  readyLabel.append(ready, element('span', text("I'm ready", '我已准备好'))); form.append(readyLabel);
  const send = element('button', text('Send status', '发送状态'), 'ready-send'); send.type = 'button'; send.disabled = true;
  const sent = element('p', '', 'status'); sent.setAttribute('role', 'status'); form.append(send, sent);
  received.append(element('h2', text('Latest update', '最近收到')));
  const result = element('strong', '—', 'ready-result'), sender = element('p', text('No update received', '尚未收到状态'), 'status');
  received.setAttribute('aria-live', 'polite');
  received.append(result, sender);
  const receiving = element('p', '', 'status'); receiving.setAttribute('role', 'status');
  const restart = element('button', text('Start receiving', '开始接收')); restart.type = 'button'; restart.hidden = true;
  const footer = element('div', '', 'ready-reception'); footer.append(receiving, restart); root.append(footer);
  let peers: MeshPeer[] = [], channel: MeshNamespace | undefined, off: (() => void) | undefined, offStatus: (() => void) | undefined;
  let disposed = false, sending = false, version = 0;
  function canSend() { send.disabled = sending || !peers.some(peer => peer.peerId === select.value); }
  select.addEventListener('change', canSend);
  function showStatus(status: MeshStatus) {
    restart.hidden = status.type !== 'error'; receiving.className = status.type === 'missed' || status.type === 'error' ? 'notice' : 'status';
    if (status.type === 'subscribing' || status.type === 'retrying') receiving.textContent = text('Connecting…', '正在连接…');
    else if (status.type === 'subscribed') receiving.textContent = text('Listening', '正在接收');
    else if (status.type === 'paused') receiving.textContent = text('Receiving paused', '已暂停接收');
    else if (status.type === 'missed') receiving.textContent = text('Some updates were missed. Ask your friend to send their status again.', '部分状态已错过，请让朋友重新发送。');
    else if (status.type === 'error') receiving.textContent = status.code === 'MESH_DISABLED' ? text('Turn on Mesh to receive updates.', '请开启 Mesh 以接收状态。') : status.code === 'PLUGIN_PERMISSION_DENIED' ? text('Send/receive permission is required.', '需要收发联系人数据的权限。') : text('Receiving is unavailable. Try again when Mesh is connected.', '暂时无法接收，Mesh 连接后可重试。');
    else if (status.type === 'handler-error') receiving.textContent = text('Could not display the update.', '未能显示收到的状态。');
    else receiving.textContent = '';
  }
  function subscribe() {
    const current = ++version; off?.(); offStatus?.(); channel?.close();
    channel = client.mesh.namespace(namespace);
    offStatus = channel.onStatus(status => { if (!disposed && version === current) showStatus(status); });
    off = channel.on('ready', (payload, message) => {
      if (disposed || version !== current) return;
      const value = readiness(payload);
      if (value === null) { receiving.textContent = text('Unsupported readiness update.', '不支持此准备状态格式。'); return; }
      result.textContent = value ? text('Ready', '已准备') : text('Not ready', '尚未准备'); result.dataset.ready = String(value);
      const name = peers.find(peer => peer.peerId === message.peerId)?.displayName || text('Friend', '朋友');
      sender.textContent = name + ' · ' + new Date(message.receivedAt * 1000).toLocaleTimeString();
    });
  }
  async function refreshPeers() {
    refresh.disabled = true;
    try {
      const value = await client.mesh.peers(); if (disposed) return;
      peers = value.peers; const selected = select.value; select.replaceChildren();
      const placeholder = element('option', text('Choose a friend', '选择朋友')); placeholder.value = ''; select.append(placeholder);
      for (const peer of peers) {
        const option = element('option', peer.displayName + (peer.connected ? '' : text(' · Not connected', ' · 未连接'))); option.value = peer.peerId; select.append(option);
      }
      select.value = peers.some(peer => peer.peerId === selected) ? selected : '';
      peerStatus.textContent = peers.length ? '' : text('Add a friend in Mesh first.', '请先在 Mesh 中添加朋友。');
    } catch (error) {
      if (disposed) return; peers = []; select.replaceChildren();
      peerStatus.textContent = errorCode(error) === 'MESH_DISABLED' ? text('Turn on Mesh to choose a friend.', '请开启 Mesh 以选择朋友。') : text('Cannot connect to friends right now.', '暂时无法连接朋友。');
    } finally { if (!disposed) { refresh.disabled = false; canSend(); } }
  }
  send.addEventListener('click', () => {
    if (send.disabled || !channel) return;
    sending = true; canSend(); sent.textContent = ''; sent.className = 'status';
    void channel.send(select.value, 'ready', { schemaVersion:1,ready:ready.checked }).then(() => { if (!disposed) sent.textContent = text('Status sent', '状态已发送'); }, error => {
      if (disposed) return; sent.className = 'error';
      const code = errorCode(error);
      sent.textContent = code === 'MESH_EVENT_DELIVERY_UNKNOWN' ? text('Delivery is unknown. Check with your friend before sending again.', '是否送达尚不明确，请先向朋友确认再发送。') : code === 'MESH_EVENT_NO_LISTENER' ? text('Ask your friend to open Ready check.', '请让朋友打开准备状态工具。') : code === 'MESH_EVENT_OFFLINE' ? text('Not connected to this friend.', '尚未连接到这位朋友。') : text('Could not send. Check the connection and try again.', '未能发送，请检查连接后重试。');
    }).finally(() => { if (!disposed) { sending = false; canSend(); } });
  });
  refresh.addEventListener('click', () => { void refreshPeers(); }); restart.addEventListener('click', subscribe);
  function close() { if (disposed) return; disposed = true; ++version; off?.(); offStatus?.(); channel?.close(); window.removeEventListener('pagehide', close); }
  window.addEventListener('pagehide', close, { once:true }); subscribe(); await refreshPeers(); return close;
}
if (typeof document !== 'undefined') {
  try { void mountReadyCheck(connect()).catch(() => { document.getElementById('app')!.textContent = 'Open this developer example in Cobble Launcher.'; }); }
  catch { document.getElementById('app')!.textContent = 'Open this developer example in Cobble Launcher.'; }
}
