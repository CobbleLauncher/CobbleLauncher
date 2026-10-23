// SPDX-License-Identifier: MIT
import { connect, type Client, type Context, type ConfigFile, type FolderReport } from '../../../../packages/plugin-web-sdk/src/index';
import './style.css';
import { populateGames } from './game-choice';
import { mountKeybindings } from './keybindings';

const messages = {
  en: { game:'Game', chooseGame:'Choose game', search:'Search configs', select:'Select visible', refresh:'Refresh', check:'Check selected', cancel:'Cancel', preview:'Source preview', results:'Results', selectFile:'Select a file to preview.', ready:'Select configs, then check.', loading:'Loading configs…', reading:'Reading config…', checking:'Checking selected configs…', cancelling:'Cancelling…', empty:'No supported configs found.', noMatches:'No matching configs.', noGame:'No games available.', listFailed:'Could not load configs. Try refreshing.', readFailed:'Could not read this config. Refresh and select it again.', checkFailed:'The check could not finish. Try again.', contextFailed:'Open this extension from CobbleLauncher.', cancelled:'Check cancelled.', previewLimit:'Preview limited to the first 64 KiB.', selected:'selected', files:'files', checked:'checked', skipped:'skipped', incomplete:'Folder scan incomplete', all:'All findings', error:'Errors', warning:'Warnings', info:'Information', noFindings:'No findings in this filter.', limit:'Select up to 64 configs.' },
  'zh-CN': { game:'游戏', chooseGame:'选择游戏', search:'搜索配置', select:'选择当前列表', refresh:'刷新', check:'检查所选', cancel:'取消', preview:'内容预览', results:'检查结果', selectFile:'选择文件以预览。', ready:'选择配置后开始检查。', loading:'正在加载配置…', reading:'正在读取配置…', checking:'正在检查所选配置…', cancelling:'正在取消…', empty:'未找到支持的配置文件。', noMatches:'没有匹配的配置。', noGame:'暂无可用游戏。', listFailed:'无法加载配置，请尝试刷新。', readFailed:'无法读取此配置，请刷新后重新选择。', checkFailed:'检查未能完成，请重试。', contextFailed:'请从 CobbleLauncher 打开此扩展。', cancelled:'已取消检查。', previewLimit:'仅预览前 64 KiB 内容。', selected:'项已选', files:'个文件', checked:'个已检查', skipped:'个已跳过', incomplete:'文件夹扫描未完成', all:'全部诊断', error:'错误', warning:'警告', info:'提示', noFindings:'当前筛选下没有诊断。', limit:'最多选择 64 个配置。' },
};
let locale: keyof typeof messages = 'en';
const t = (key: keyof typeof messages.en) => messages[locale][key];
const element = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const game = element<HTMLSelectElement>('game');
const search = element<HTMLInputElement>('search');
const selectAll = element<HTMLInputElement>('select-all');
const refresh = element<HTMLButtonElement>('refresh');
const check = element<HTMLButtonElement>('check');
const cancel = element<HTMLButtonElement>('cancel');
const severity = element<HTMLSelectElement>('severity');
let client: Client;
let context: Context;
let files: ConfigFile[] = [];
let selected = new Set<string>();
let active = '';
let reading = 0;
let generation = 0;
let checking = 0;
let busy = false;
let listing = false;
let result: FolderReport | null = null;
function text(id: string, value: string) { element(id).textContent = value; }
function error(message = '') { text('error', message); element('error').hidden = !message; }
function status(id: string, message: string, loading = false) { text(id,message); element(id).classList.toggle('loading',loading); }
function node<K extends keyof HTMLElementTagNameMap>(tag: K, value = '', className = '') { const e=document.createElement(tag); e.textContent=value; e.className=className; return e; }
function size(bytes: number) { return bytes < 1024 ? `${bytes} B` : `${(bytes/1024).toFixed(1)} KiB`; }
function matches() { const query=search.value.trim().toLocaleLowerCase(); return files.filter(file=>file.name.toLocaleLowerCase().includes(query)); }
function updateActions() {
  const visible=matches();
  check.disabled=busy||listing||!selected.size||!game.value;
  check.textContent=`${t('check')}${selected.size ? ` (${selected.size})` : ''}`;
  refresh.disabled=busy||listing||!game.value; search.disabled=listing||!game.value;
  selectAll.disabled=busy||listing||!visible.length;
  selectAll.checked=visible.length>0&&visible.every(file=>selected.has(file.id));
  selectAll.indeterminate=!selectAll.checked&&visible.some(file=>selected.has(file.id));
  text('count',`${files.length} ${t('files')} · ${selected.size} ${t('selected')}`);
  game.disabled=!!context?.instanceId||busy||!context?.games.length;
  cancel.hidden=!busy;
}
function renderFiles() {
  const list=element('files'); list.replaceChildren();
  for(const file of matches()) {
    const row=node('li'); row.classList.toggle('active',active===file.id);
    const label=node('label'); const checkbox=node('input'); checkbox.type='checkbox';checkbox.checked=selected.has(file.id);checkbox.disabled=busy;
    checkbox.setAttribute('aria-label',`${t('check')}: ${file.name}`);
    checkbox.onchange=()=>{if(checkbox.checked){if(selected.size>=64){checkbox.checked=false;error(t('limit'));return;}selected.add(file.id);}else selected.delete(file.id);updateActions();};
    label.append(checkbox);
    const open=node('button');open.type='button';open.title=file.name;open.append(node('span',file.name),node('small',size(file.size)));open.onclick=()=>void read(file);
    row.append(label,open);list.append(row);
  }
  if(!listing)status('list-status',files.length ? (matches().length ? '' : t('noMatches')) : t('empty'));
  updateActions();
}
async function inventory() {
  const epoch=++generation;const id=game.value;reading++;active='';
  listing=true;error();status('list-status',t('loading'),true);updateActions();
  try { const value=await client.configs.list(id);if(epoch!==generation)return;files=value;selected=new Set([...selected].filter(id=>files.some(file=>file.id===id))); }
  catch {if(epoch===generation){error(t('listFailed'));}}
  finally {if(epoch===generation){listing=false;status('list-status','');renderFiles();}}
}
async function read(file: ConfigFile) {
  const epoch=generation;const request=++reading;const id=game.value;
  active=file.id;renderFiles();text('preview-title',file.name);text('preview-size',size(file.size));element('source').hidden=true;element('preview-note').hidden=true;status('preview-status',t('reading'),true);
  try {const doc=await client.configs.read(id,file.id);if(epoch!==generation||request!==reading)return;if(doc.id!==file.id)throw new Error('CONFIG_ID_MISMATCH');
    // The source is a text node, never HTML. A bounded preview avoids a huge DOM.
    const bytes=new TextEncoder().encode(doc.text);let preview=doc.text;
    if(bytes.length>65536){preview=new TextDecoder('utf-8',{fatal:false}).decode(bytes.subarray(0,65536),{stream:true});element('preview-note').hidden=false;text('preview-note',t('previewLimit'));}
    text('source',preview);element('source').hidden=false;status('preview-status','');
  } catch {if(epoch===generation&&request===reading)status('preview-status',t('readFailed'));}
}
function renderReport() {
  const host=element('report');host.replaceChildren();
  if(!result)return;
  const report=result.report;
  const findings=report.findings.filter(f=>severity.value==='all'||f.severity===severity.value);
  for(const finding of findings){const card=node('article','','finding');card.dataset.severity=finding.severity;card.append(node('span',t(finding.severity),'badge'),node('h3',finding.message||finding.title));for(const suggestion of finding.suggestions)card.append(node('p',suggestion));host.append(card);}
  if(!findings.length)host.append(node('p',t('noFindings'),'muted'));
  for(const table of report.tables){const box=node('div','','table-scroll');const view=node('table');const head=node('thead');const row=node('tr');for(const column of table.columns){const cell=node('th',column);cell.scope='col';row.append(cell);}head.append(row);view.append(head);const body=node('tbody');for(const values of table.rows){const row=node('tr');for(const value of values)row.append(node('td',value));body.append(row);}view.append(body);box.append(view);host.append(box);}
}
async function run() {
  const epoch=generation;const request=++checking;busy=true;result=null;error();element('report').replaceChildren();severity.hidden=true;status('result-status',t('checking'),true);renderFiles();
  try {const value=await client.report.check(game.value,[...selected]);if(epoch!==generation||request!==checking)return;result=value;
    if(!value){status('result-status',t('cancelled'));return;}
    status('result-status',`${value.files} ${t('checked')} · ${value.skipped} ${t('skipped')}${value.truncated ? ` · ${t('incomplete')}` : ''}`);severity.hidden=false;renderReport();
  }catch {if(epoch===generation&&request===checking)status('result-status',t('checkFailed'));}
  finally {if(request===checking){busy=false;renderFiles();}}
}
function applyContext(value: Context) {
  context=value;locale=value.locale==='zh-CN'?'zh-CN':'en';document.documentElement.lang=locale;document.documentElement.dataset.theme=value.theme==='system'?(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'):value.theme;
  text('game-label',t('game'));text('select-label',t('select'));text('refresh',t('refresh'));text('cancel',t('cancel'));text('preview-title',t('preview'));text('results-title',t('results'));search.placeholder=t('search');search.setAttribute('aria-label',t('search'));severity.setAttribute('aria-label',t('results'));
  severity.replaceChildren(...(['all','error','warning','info'] as const).map(key=>new Option(t(key),key)));
  populateGames(game, value, t('chooseGame'));
  const canSwitch=mountKeybindings(client,value,element('bindings-panel'));
  text('configs-tab',locale==='zh-CN'?'配置检查':'Configs');text('bindings-tab',locale==='zh-CN'?'按键复制':'Keybindings');
  function switchTab(bindings:boolean){if(busy||!canSwitch())return;element('configs-panel').hidden=bindings;element('bindings-panel').hidden=!bindings;element('configs-tab').setAttribute('aria-pressed',String(!bindings));element('bindings-tab').setAttribute('aria-pressed',String(bindings));game.parentElement!.hidden=bindings;}
  element('configs-tab').onclick=()=>switchTab(false);element('bindings-tab').onclick=()=>switchTab(true);
  if(value.keybindingTargetId)switchTab(true);
  status('preview-status',t('selectFile'));status('result-status',t('ready'));updateActions();
}
search.oninput=renderFiles;
selectAll.onchange=()=>{if(selectAll.checked){for(const file of matches()){if(selected.size>=64)break;selected.add(file.id);}}else for(const file of matches())selected.delete(file.id);renderFiles();};
refresh.onclick=()=>void inventory();check.onclick=()=>void run();severity.onchange=renderReport;
game.onchange=()=>{selected.clear();files=[];result=null;element('source').hidden=true;element('preview-note').hidden=true;text('preview-title',t('preview'));text('preview-size','');status('preview-status',t('selectFile'));status('result-status',t('ready'));element('report').replaceChildren();severity.hidden=true;void inventory();};
cancel.onclick=async()=>{cancel.disabled=true;status('result-status',t('cancelling'),true);try{await client.report.cancel();}catch{error(t('checkFailed'));}finally{cancel.disabled=false;}};
window.addEventListener('pagehide',()=>{generation++;reading++;checking++;if(busy)void client?.report.cancel().catch(()=>{});});
async function start(){try{client=connect();applyContext(await client.context());if(game.value)await inventory();else status('list-status',context.games.length ? '' : t('noGame'));}catch{error(t('contextFailed'));}}
void start();
