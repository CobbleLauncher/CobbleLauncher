// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext,connect,type Report} from '../../../../packages/plugin-web-sdk/src/index';
import {counters,time,distance,percent,label,type Counter} from './model';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
const host=document.getElementById('app')!;host.className='workbench chronicle';
function mount(client: ReturnType<typeof connect>){let zh=false,epoch=0,busy=false,report:Report|null=null;
const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const tr=(en:string,cn:string)=>zh?cn:en;
function render(){
 host.replaceChildren();host.setAttribute('aria-busy',String(busy));
 const head=el('header','','toolbar'),titles=el('div'),title=el('h1',tr('World Chronicle','世界游玩年鉴'));titles.append(title);

 const actions=el('div','','actions'),pick=el('button',tr(report?'Choose another file':'Choose stats JSON',report?'选择其他文件':'选择统计 JSON'),'primary');pick.disabled=busy;pick.onclick=()=>void choose();actions.append(pick);
 if(busy){const cancel=el('button',tr('Cancel','取消'));cancel.onclick=()=>{epoch++;busy=false;void client.report.cancel().catch(()=>{});render();message(tr('Cancelled.','已取消。'));};actions.append(cancel);}head.append(titles,actions);host.append(head);
 if(busy){const state=el('p',tr('Reading statistics…','正在读取统计…'),'status');state.role='status';host.append(state);}
 if(!report){const empty=el('section','','welcome');empty.append(el('p',tr('Choose a player statistics JSON from a world’s stats folder.','从世界的 stats 文件夹选择一份玩家统计 JSON。')));host.append(empty);return;}
 const custom=counters(report,'custom'),get=(id:string)=>custom.find(r=>r.id===`minecraft:${id}`)?.value;
 const metrics=el('section','','hero-metrics');
 function metric(name:string,value:string){const card=el('div','','metric');card.append(el('span',name),el('strong',value));metrics.append(card);}
 const play=get('play_time')??get('play_one_minute');metric(tr('Time played','游玩时间'),play===undefined?'—':time(play,zh));
 const foot=['walk_one_cm','sprint_one_cm','crouch_one_cm'].map(get),present=foot.filter((v):v is bigint=>v!==undefined);metric(tr('On foot','步行与奔跑'),present.length?distance(present.reduce((a,b)=>a+b,0n),zh):'—');
 metric(tr('Mob kills','生物击杀'),get('mob_kills')?.toLocaleString(zh?'zh-CN':'en')??'—');metric(tr('Deaths','死亡次数'),get('deaths')?.toLocaleString(zh?'zh-CN':'en')??'—');host.append(metrics);
 const grid=el('div','','chronicle-grid');
 function ranking(title:string,rows:Counter[],unit:'count'|'distance'='count'){
 const box=el('section','','ranking'),heading=el('h2',title);box.append(heading);if(!rows.length){box.append(el('p',tr('Not recorded in this file.','此文件没有记录。'),'muted'));grid.append(box);return;}
 const max=rows.reduce((m,r)=>r.value>m?r.value:m,0n);for(const row of rows){const item=el('div','','rank-row'),top=el('div','','rank-label'),name=el('span',unit==='distance'?movementName(row.id):label(row.id)),value=el('strong',unit==='distance'?distance(row.value,zh):row.value.toLocaleString(zh?'zh-CN':'en'));name.title=row.id;top.append(name,value);const track=el('div','','bar-track'),bar=el('div','','bar');bar.style.width=`${percent(row.value,max)}%`;track.append(bar);item.append(top,track);box.append(item);}grid.append(box);}
 ranking(tr('Most mined','挖掘最多'),counters(report,'mined'));ranking(tr('Creatures defeated','击败的生物'),counters(report,'killed'));
 ranking(tr('Ways you travelled','你的移动方式'),custom.filter(r=>r.id.endsWith('_one_cm')).sort((a,b)=>a.value>b.value?-1:a.value<b.value?1:0),'distance');host.append(grid);
 for(const finding of report.findings){const note=el('p',`${finding.title} · ${finding.message}`,'notice');note.role='status';host.append(note);}
 host.append(el('p',tr('Statistics from the selected file. — means unrecorded.','数据来自所选文件。— 表示未记录。'),'footnote'));
}
function movementName(id:string){const names:Record<string,[string,string]>={walk_one_cm:['Walking','步行'],sprint_one_cm:['Sprinting','奔跑'],crouch_one_cm:['Sneaking','潜行'],swim_one_cm:['Swimming','游泳'],fly_one_cm:['Flying','飞行'],aviate_one_cm:['Elytra','鞘翅'],boat_one_cm:['Boat','船'],minecart_one_cm:['Minecart','矿车'],horse_one_cm:['Horse','骑马']};const value=names[id.replace('minecraft:','')];return value?value[zh?1:0]:label(id);}
function message(text:string,error=false){const node=el('p',text,error?'error':'status');node.role=error?'alert':'status';host.insertBefore(node,host.children[1]??null);}
async function choose(){const generation=++epoch;busy=true;render();try{const result=await client.report.pick();if(generation!==epoch)return;busy=false;if(result)report=result;render();if(!result)message(tr('No file selected.','未选择文件。'));}catch(cause){if(generation!==epoch)return;busy=false;render();const code=String(cause);message(/PERMISSION/i.test(code)?tr('Allow selected-file reading in this tool’s permissions.','请在此工具的权限中允许读取所选文件。'):tr('Could not read these statistics. Choose a Minecraft stats JSON file up to 2 MiB.','无法读取这些统计，请选择不超过 2 MiB 的 Minecraft 统计 JSON。'),true);}}
window.addEventListener('keydown',e=>{if(e.key==='Escape'&&busy){epoch++;busy=false;void client.report.cancel().catch(()=>{});render();message(tr('Cancelled.','已取消。'));}});
void hostContext(client.context()).then(value=>{zh=value.locale==='zh-CN';document.documentElement.lang=zh?'zh-CN':'en';document.documentElement.dataset.theme=value.theme==='system'?(matchMedia('(prefers-color-scheme:dark)').matches?'dark':'light'):value.theme;render();}).catch(()=>render());
render();

}
try{mount(connect());}catch{host.append(document.createTextNode('Open World Chronicle from the launcher. / 请从启动器打开世界游玩年鉴。'));}

installPluginPresentation(presentation);
