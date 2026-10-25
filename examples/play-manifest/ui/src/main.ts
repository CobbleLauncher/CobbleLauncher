// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext,connect,type Client,type Context} from '../../../../packages/plugin-web-sdk/src/index';
import type {PlayManifestPreview} from '../../../../packages/plugin-web-sdk/src/play-manifest';
import {canExport} from './model';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
export async function mountPlayManifest(client:Client,host=document.getElementById('app')!){
 let context:Context;try{context=await hostContext(client.context());}catch{host.textContent='Could not load games / 无法加载游戏';return;}
 const zh=context.locale==='zh-CN';document.documentElement.lang=zh?'zh-CN':'en';document.documentElement.dataset.theme=context.theme;
 const t=(en:string,cn:string)=>zh?cn:en;let preview:PlayManifestPreview|null=null,busy=false;const selected=new Set<string>();
 const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
 function icon(kind:'file'|'box'|'excluded') {const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('viewBox','0 0 24 24');svg.setAttribute('fill','none');svg.setAttribute('stroke','currentColor');svg.setAttribute('stroke-width','1.6');svg.setAttribute('aria-hidden','true');const path=document.createElementNS(svg.namespaceURI,'path');path.setAttribute('d',kind==='file'?'M6 3h8l4 4v14H6z M14 3v5h4 M9 12h6 M9 16h6':kind==='box'?'M4 7l8-4 8 4v10l-8 4-8-4z M4 7l8 4 8-4 M12 11v10':'M5 5l14 14 M19 5L5 19');svg.append(path);return svg;}
 const title=el('h1',t('Share play configuration','分享开玩配置'));const select=el('select');select.setAttribute('aria-label',t('Game','游戏'));const blank=el('option',t('Choose a game','选择游戏'));blank.value='';select.append(blank);
 for(const game of context.games){const option=el('option',game.name);option.value=game.id;select.append(option);}select.value=context.instanceId??'';
 const load=el('button',t('Preview','预览'),'secondary');const header=el('header');header.append(title,select,load);
 const status=el('p','','status');status.setAttribute('role','status');const content=el('section');
 const footer=el('footer');const confirmations=el('div','','confirmations');const reviewed=el('input');reviewed.type='checkbox';const reviewLabel=el('label');reviewLabel.append(reviewed,document.createTextNode(t('Selected configurations checked for private data','已检查所选配置中的隐私信息')));
 const omitted=el('input');omitted.type='checkbox';const omittedLabel=el('label');omittedLabel.append(omitted,document.createTextNode(t('Continue without excluded Mods','不包含已排除的 Mod，继续导出')));
 const save=el('button',t('Save .mrpack…','保存 .mrpack…'),'primary');const exportSummary=el('span','','export-summary');const exportAction=el('div','','export-action');exportAction.append(exportSummary,save);confirmations.append(reviewLabel,omittedLabel);footer.append(confirmations,exportAction);footer.hidden=true;host.replaceChildren(header,status,content,footer);
 function buttons(){load.disabled=busy||!select.value;select.disabled=busy;save.disabled=!canExport(preview,busy,reviewed.checked,omitted.checked);exportSummary.textContent=t(`${preview?.mods.length??0} Mod ${(preview?.mods.length??0)===1?'reference':'references'} · ${selected.size} ${selected.size===1?'configuration':'configurations'}`,`${preview?.mods.length??0} 个 Mod 引用 · ${selected.size} 个配置`);}
 function reset(){preview=null;selected.clear();reviewed.checked=false;omitted.checked=false;content.replaceChildren();footer.hidden=true;status.textContent='';buttons();}
 function fail(error:unknown){const code=String(error);status.textContent=/LIMIT/.test(code)?t('Sharing limit: 1024 Mods, 2 GiB; 64 text configurations, 512 KiB.','分享上限：1024 个 Mod、2 GiB；64 个文本配置、512 KiB。'):/UNSAFE_FILE/.test(code)?t('Linked or unsupported files cannot be shared.','无法分享链接文件或不支持的文件。'):/STALE/.test(code)?t('The game changed. Preview again.','游戏内容已变化，请重新预览。'):/PERMISSION|REVOKED/.test(code)?t('Allow play-configuration export in this tool’s permissions.','请在此工具权限中允许开玩配置导出。'):/关闭|运行|BUSY/.test(code)?t('Close the game and finish current tasks first.','请先关闭游戏并等待当前任务完成。'):t('Could not prepare or save this configuration. Try Preview again.','无法准备或保存配置，请重新预览。');}
 function render(){
  if(!preview)return;content.replaceChildren();const game=el('div','','game-summary');game.append(icon('box'),el('strong',preview.name),el('span',`Minecraft ${preview.version}`));if(preview.loader)game.append(el('span',`${preview.loader} ${preview.loaderVersion??''}`));content.append(game);
  const sources=el('div','','sources');const mods=el('section','','source-card');const modHeading=el('div','','card-heading');modHeading.append(icon('box'),el('h2',t('Mod references','Mod 引用')),el('span',String(preview.mods.length),'count'));mods.append(modHeading);
  const modList=el('ul','','mod-list');for(const mod of preview.mods){const row=el('li');const source=el('a',mod.name);source.href=`https://modrinth.com/mod/${encodeURIComponent(mod.projectId)}/version/${encodeURIComponent(mod.versionId)}`;source.target='_blank';source.rel='noreferrer';row.append(source,el('span',mod.source,'source-badge'));modList.append(row);}mods.append(modList);if(!preview.mods.length)mods.append(el('p',t('No verified download references.','没有已验证的下载引用。'),'muted'));sources.append(mods);
  if(preview.excludedMods.length){const excluded=el('section','','source-card excluded');const heading=el('div','','card-heading');heading.append(icon('excluded'),el('h2',t('Excluded Mods','已排除的 Mod')),el('span',String(preview.excludedMods.length),'count'));excluded.append(heading);const list=el('ul','','mod-list');for(const name of preview.excludedMods)list.append(el('li',name));excluded.append(list,el('p',t('No verified download reference','未找到已验证的下载引用'),'muted'));sources.append(excluded);}content.append(sources);
  const configurations=el('section','','configurations');const heading=el('div','','config-heading');heading.append(el('h2',t('Configuration','配置')),el('span',t('Select files to include','选择需要包含的文件'),'muted'));configurations.append(heading);
  for(const config of preview.configs){const row=el('article','','config-row');const top=el('div','','config-top');const choice=el('label','','config-choice');const box=el('input');box.type='checkbox';box.setAttribute('aria-label',config.name);choice.append(box,icon('file'),el('span',config.name,'filename'));
   const body=el('pre',config.text);body.hidden=true;body.id=`config-${config.id}`;const toggle=el('button',t('Review','查看全文'),'review');toggle.setAttribute('aria-expanded','false');toggle.setAttribute('aria-controls',body.id);toggle.onclick=()=>{body.hidden=!body.hidden;toggle.textContent=body.hidden?t('Review','查看全文'):t('Collapse','收起');toggle.setAttribute('aria-expanded',String(!body.hidden));};
   box.onchange=()=>{if(box.checked)selected.add(config.id);else selected.delete(config.id);reviewed.checked=false;row.classList.toggle('selected',box.checked);buttons();};top.append(choice,el('span',`${Math.max(1,Math.ceil(config.size/1024))} KB`,'file-size'),toggle);row.append(top,body);configurations.append(row);
  }
  if(!preview.configs.length)configurations.append(el('p',t('No supported text configuration files.','没有支持的文本配置文件。'),'muted'));content.append(configurations,el('p',t('Accounts, worlds, screenshots and logs excluded. Mods download from the original source on import.','排除账号、世界、截图和日志。导入时从原始来源下载 Mod。'),'privacy-note'));footer.hidden=false;omittedLabel.hidden=!preview.excludedMods.length;buttons();
 }
 async function inspect(){reset();busy=true;buttons();status.textContent=t('Preparing preview…','正在准备预览…');try{preview=await client.playManifest.preview(select.value);status.textContent='';render();}catch(error){fail(error);}finally{busy=false;buttons();}}
 load.onclick=()=>void inspect();select.onchange=reset;reviewed.onchange=buttons;omitted.onchange=buttons;
 save.onclick=async()=>{if(!preview||save.disabled)return;busy=true;buttons();try{const result=await client.playManifest.export({gameId:select.value,revision:preview.revision,configIds:[...selected]});status.textContent=result.saved?t('Saved. Send the .mrpack to your friend; review it from Library → Import.','已保存。把 .mrpack 发给朋友，通过「游戏库 → 导入」预览后安装。'):t('Save cancelled.','已取消保存。');}catch(error){fail(error);}finally{busy=false;buttons();}};
 buttons();if(select.value)await inspect();
}
if((globalThis as {__COBBLE_PLUGIN_BRIDGE__?:unknown}).__COBBLE_PLUGIN_BRIDGE__)void mountPlayManifest(connect());


installPluginPresentation(presentation);
