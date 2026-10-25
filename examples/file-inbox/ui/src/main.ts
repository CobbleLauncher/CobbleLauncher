// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext,connect,type Client,type FileInboxItem} from '../../../../packages/plugin-web-sdk/src/index';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
export async function mountInbox(client:Client,host=document.getElementById('app')!){
 const context=await hostContext(client.context());document.documentElement.lang=context.locale;document.documentElement.dataset.theme=context.theme;let zh=context.locale==='zh-CN',items:FileInboxItem[]=[],active='',handler='',game='',generation=0,busy=false,message='',failure='';
 const tr=(en:string,cn:string)=>zh?cn:en;
 const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='',cls='')=>{const n=document.createElement(tag);n.textContent=text;n.className=cls;return n;};
 const button=(text:string,action:()=>void,cls='')=>{const n=el('button',text,cls);n.type='button';n.onclick=action;return n;};
 const kind=(v:string)=>({mrpack:tr('Modrinth pack','Modrinth 整合包'),'curseforge-pack':tr('CurseForge pack','CurseForge 整合包'),mod:'Mod','resource-pack':tr('Resource pack','资源包'),'shader-pack':tr('Shader pack','光影包'),blueprint:tr('Blueprint','蓝图'),log:tr('Log','日志'),ambiguous:tr('Choose a format','请选择格式'),unknown:tr('Unknown format','未知格式')}[v]||v);
 const size=(n:number)=>n<1048576?`${(n/1024).toFixed(1)} KB`:`${(n/1048576).toFixed(1)} MB`;
 function select(id:string){active=id;handler='';game='';message='';failure='';render();}
 function cancel(){generation++;busy=false;message=tr('Cancelled.','已取消。');render();}
 async function load(pick=false){const token=++generation;busy=true;failure='';message='';render();try{const next=await(pick?client.fileInbox.pick():client.fileInbox.pending());if(token!==generation)return;items=next;if(!items.some(i=>i.resourceId===active)){active=items[0]?.resourceId||'';handler='';game='';}}catch(e){if(token===generation)failure=error(e);}finally{if(token===generation){busy=false;render();}}}
 function error(e:unknown){const code=String(e);return /PERMISSION/.test(code)?tr('Allow file import requests in this plugin’s permissions.','请在此插件权限中允许文件导入请求。'):/LIMIT/.test(code)?tr('Choose up to 8 files, at most 512 MB each and 1 GB total.','最多选择 8 个文件，每个不超过 512 MB，总计不超过 1 GB。'):/EXPIRED/.test(code)?tr('This file expired. Choose it again.','文件已过期，请重新选择。'):/ACTION_PENDING/.test(code)?tr('Complete the open review first.','请先完成已打开的确认。'):tr('Could not complete this request. Choose the file again.','无法完成此请求，请重新选择文件。');}
 async function release(id:string){const token=++generation;busy=true;render();try{await client.fileInbox.release({resourceIds:[id]});if(token!==generation)return;items=items.filter(i=>i.resourceId!==id);if(active===id)select(items[0]?.resourceId||'');}catch(e){failure=error(e);}finally{if(token===generation){busy=false;render();}}}
 async function open(){const item=items.find(i=>i.resourceId===active),chosen=item?.handlers.find(h=>h.id===handler);if(!item||!chosen||chosen.requiresGame&&!game)return;const token=++generation;busy=true;failure='';render();try{await client.fileInbox.open({resourceId:active,handlerId:handler,...(chosen.requiresGame?{gameId:game}:{})});if(token===generation)message=tr('Review requested in the launcher.','已请求在启动器中查看。');}catch(e){if(token===generation)failure=error(e);}finally{if(token===generation){busy=false;render();}}}
 function render(){host.replaceChildren();host.className='workbench file-inbox';host.dataset.theme=context.theme;
 const header=el('header','','toolbar');header.append(el('h1',tr('File Inbox','文件投递')));const actions=el('div','','actions');actions.append(button(tr('Choose files','选择文件'),()=>void load(true),'primary'));if(busy)actions.append(button(tr('Cancel','取消'),cancel));else actions.append(button(tr('Refresh','刷新'),()=>void load()));header.append(actions);host.append(header);
 if(message||busy){const status=el('p',busy?tr('Loading…','正在加载…'):message,'status');status.role='status';status.setAttribute('aria-live','polite');host.append(status);}if(failure){const n=el('p',failure,'error');n.role='alert';host.append(n);}
 if(!items.length){const empty=el('section','','empty-inbox');empty.append(el('div','↓','drop-symbol'),el('h2',tr('Bring a file','投递文件')),el('p',tr('Drop files into the launcher or choose files.','将文件拖入启动器，或选择文件。')),el('p',tr('Modpacks · Mods · Resource packs · Blueprints · Logs','整合包 · Mod · 资源包 · 蓝图 · 日志'),'muted'));host.append(empty);return;}
 const layout=el('div','','inbox-layout'),queue=el('nav','','queue'),detail=el('section','','inbox-detail');queue.setAttribute('aria-label',tr('Selected files','所选文件'));queue.append(el('h2',tr(`Files (${items.length})`,`文件 (${items.length})`)));
 for(const item of items){const row=button('',()=>select(item.resourceId),'file-row');row.setAttribute('aria-current',String(item.resourceId===active));row.append(el('strong',item.name),el('span',`${kind(item.kind)} · ${size(item.bytes)}`));queue.append(row);}
 const item=items.find(i=>i.resourceId===active);if(item){const top=el('div','','detail-heading');top.append(el('h2',item.name),button(tr('Remove','移除'),()=>void release(item.resourceId)));detail.append(top,el('p',`${kind(item.kind)} · ${size(item.bytes)}`,'muted'));
 if(item.declaredMinecraft||item.declaredLoader)detail.append(el('p',[item.declaredMinecraft?`Minecraft ${item.declaredMinecraft}`:'',item.declaredLoader||''].filter(Boolean).join(' · '),'metadata'));
 if(item.kind==='ambiguous')detail.append(el('p',tr('Several formats were found. Choose how to open this file.','发现多种格式，请选择打开方式。'),'notice'));
 if(item.error)detail.append(el('p',tr('The file could not be inspected.','无法检查此文件。'),'error'));
 if(!item.handlers.length)detail.append(el('p',tr('No enabled handler supports this file.','没有已启用的处理器支持此文件。'),'notice'));
 else{const field=el('fieldset'),legend=el('legend',tr('Open with','打开方式'));field.append(legend);for(const h of item.handlers){const label=el('label','','handler'),radio=el('input');radio.type='radio';radio.name='handler';radio.value=h.id;radio.checked=handler===h.id;radio.onchange=()=>{handler=h.id;game='';render();};label.append(radio,el('span',h.label),el('small',h.kind==='import'?tr('Import','导入'):h.kind==='viewer'?tr('View','查看'):tr('Analyze','分析')));field.append(label);}detail.append(field);
 const chosen=item.handlers.find(h=>h.id===handler);if(chosen?.requiresGame){const label=el('label',tr('Target game','目标游戏'),'game-label'),select=el('select');select.setAttribute('aria-label',tr('Target game','目标游戏'));const placeholder=el('option',tr('Choose a game','选择游戏'));placeholder.value='';select.append(placeholder);for(const g of item.games){const o=el('option',`${g.name} · ${g.version}${!g.available?tr(' · Running',' · 运行中'):g.compatibility==='mismatch'?tr(' · Version differs',' · 版本不同'):''}`);o.value=g.id;o.disabled=!g.available;select.append(o);}select.value=game;select.onchange=()=>{game=select.value;render();};label.append(select);detail.append(label);const target=item.games.find(g=>g.id===game);if(target)detail.append(el('p',target.compatibility==='declared-match'?tr('Same declared Minecraft version.','声明的 Minecraft 版本相同。'):target.compatibility==='mismatch'?tr('Check the file’s required version before importing.','导入前请检查文件要求的版本。'):tr('Compatibility is not declared. Check the file’s requirements.','兼容性未声明，请检查文件要求。'),'muted'));}
 const submit=button(tr('Continue','继续'),()=>void open(),'primary review');submit.disabled=busy||!chosen||!!chosen.requiresGame&&!game;detail.append(submit);}
 }layout.append(queue,detail);host.append(layout);for(const b of host.querySelectorAll('button'))if(b.textContent!==tr('Cancel','取消')&&busy)b.disabled=true;}
 host.addEventListener('keydown',event=>{if(event.key==='Escape'&&busy)cancel();});render();await load();return{cancel,refresh:()=>load(),destroy:()=>{generation++;host.replaceChildren();}};
}
try{await mountInbox(connect());}catch{const host=document.getElementById('app');if(host)host.textContent='Open File Inbox from the launcher. / 请从启动器打开文件投递。';}



installPluginPresentation(presentation);
