// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { hostContext,connect,type Client,type Context,type Report} from '../../../../packages/plugin-web-sdk/src/index';
import {boundedNeighborhood,dependencyGraph,filterMods,parseInventory,selectedNode,type Filter,type Graph} from './model';
import {renderGraph} from './graph';
import {renderDetails} from './details';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';

const host=document.querySelector<HTMLElement>('#app')!;
const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const toolbar=el('header','','toolbar'),game=el('select'),refresh=el('button'),cancel=el('button'),actions=el('div','','actions');
refresh.dataset.action='refresh';actions.append(refresh,cancel);toolbar.append(game,actions);
const status=el('p','','status'),error=el('p','','error'),notice=el('p','','notice'),stats=el('div','','stats');
status.setAttribute('role','status');status.setAttribute('aria-live','polite');error.setAttribute('role','alert');error.hidden=true;notice.hidden=true;
const filters=el('div','','filters'),search=el('input'),filter=el('select'),count=el('span','','muted');search.type='search';filters.append(search,filter,count);
const workspace=el('div','','dependency-workspace'),inventory=el('section','','mod-list inventory'),list=el('div'),more=el('button'),center=el('section','','map-panel inventory'),mapToolbar=el('div','','section-heading'),mapTitle=el('h2'),expand=el('button'),reset=el('button'),mapCount=el('span','','muted'),map=el('div','','dependency-map'),details=el('aside','','mod-details analysis');
expand.dataset.action='expand-neighborhood';inventory.append(list,more);mapToolbar.append(mapTitle,mapCount,expand,reset);map.tabIndex=0;center.append(mapToolbar,map);workspace.append(inventory,center,details);
const analysis=el('details','','analysis-summary'),summary=el('summary'),findings=el('div');analysis.append(summary,findings);
host.append(toolbar,error,status,notice,stats,filters,workspace,analysis);
let client:Client,context:Context,zh=false,graph:Graph=dependencyGraph([]),selection:string|null=null,epoch=0,loading=false,checking=false,budget=40,listLimit=100,seeds:string[]=[],report:Report|null=null;
const tr=(en:string,cn:string)=>zh?cn:en;
function labels(){
  game.setAttribute('aria-label',tr('Game','游戏'));refresh.textContent=tr('Refresh','刷新');cancel.textContent=tr('Cancel','取消');search.placeholder=tr('Search Mods or dependencies','搜索模组或依赖');search.setAttribute('aria-label',search.placeholder);filter.setAttribute('aria-label',tr('Filter Mods','筛选模组'));
  filter.replaceChildren(...([['all','All Mods','全部模组'],['enabled','Enabled','已启用'],['disabled','Disabled','已禁用'],['requirements','Unmatched requirements','未匹配要求'],['unknown','Unknown metadata','元数据未知']] as const).map(([value,en,cn])=>new Option(tr(en,cn),value)));
  mapTitle.textContent=tr('Mod → dependency','模组 → 依赖');expand.textContent=tr('Expand neighborhood','展开相邻项');reset.textContent=tr('Reset map','重置依赖图');summary.textContent=tr('Analysis','分析');more.textContent=tr('Show more Mods','显示更多模组');
  map.setAttribute('aria-label',tr('Dependency map. Tab to nodes; Enter selects.','依赖图。按 Tab 切换节点，按 Enter 选择。'));ui();
}
function ui(){host.setAttribute('aria-busy',String(loading));game.disabled=loading||!!context?.instanceId;refresh.disabled=loading||!game.value;cancel.hidden=!loading&&!checking;}
function choose(id:string){const fromMap=document.activeElement?.classList.contains('graph-node'),fromList=document.activeElement?.classList.contains('mod-row');if(graph.nodes.get(id)?.mod&&!filterMods(graph,search.value,filter.value as Filter).some(node=>node.id===id)){search.value='';filter.value='all';}selection=id;seeds=[id];budget=40;render();if(fromMap)map.querySelector<HTMLButtonElement>('[data-selected=true]')?.focus({preventScroll:true});else if(fromList)list.querySelector<HTMLButtonElement>('[data-selected=true]')?.focus({preventScroll:true});else details.querySelector('h2')?.scrollIntoView({block:'nearest'});}
function render(){
  const visible=filterMods(graph,search.value,filter.value as Filter);selection=selectedNode(graph,visible,selection);count.textContent=`${visible.length} / ${[...graph.nodes.values()].filter(n=>n.mod).length}`;
  list.replaceChildren();
  for(const node of visible.slice(0,listLimit)){
    const button=el('button','','mod-row'),name=el('strong',node.name),meta=el('span',[node.mod?.version??tr('Unknown version','版本未知'),node.mod?.disabled?tr('Disabled','已禁用'):''].filter(Boolean).join(' · '),'muted');button.dataset.selected=String(node.id===selection);button.setAttribute('aria-pressed',String(node.id===selection));button.append(name,meta);button.onclick=()=>choose(node.id);list.append(button);
  }
  if(!visible.length)list.append(el('p',search.value?tr('No matches.','没有匹配项。'):tr('No Mods in this filter.','此筛选下没有模组。'),'empty'));more.hidden=visible.length<=listLimit;
  const bounded=boundedNeighborhood(graph,seeds.length?seeds:selection?[selection]:visible.slice(0,1).map(n=>n.id),budget);
  const nodesCount=bounded.omitted?`${bounded.nodes.length} / ${bounded.totalNodes}`:String(bounded.nodes.length);
  mapCount.textContent=tr(`${nodesCount} nodes`,`${nodesCount} 个节点`)+(bounded.omittedEdges?tr(` · ${bounded.omittedEdges} edges hidden`,` · ${bounded.omittedEdges} 条连线未显示`):'');expand.disabled=(!bounded.omitted&&!bounded.omittedEdges)||budget>=160;reset.disabled=budget===40;
  renderGraph(map,bounded.nodes,bounded.facts,selection,zh,choose);renderDetails(details,graph,selection,zh,choose);
  stats.replaceChildren();
  const mods=[...graph.nodes.values()].filter(node=>node.mod),unmatched=graph.facts.filter(fact=>!fact.optional&&['missing','disabled'].includes(fact.status));
  for(const [value,label] of [[mods.length,tr('Mods','模组')],[graph.facts.length,tr('Declared dependencies','已声明依赖')],[unmatched.length,tr('Unmatched requirements','未匹配要求')]]){const metric=el('div','','metric');metric.append(el('strong',String(value)),el('span',String(label)));stats.append(metric);}
}
function renderReport(){
  summary.textContent=checking?tr('Analyzing…','正在分析…'):tr('Analysis','分析');findings.replaceChildren();
  if(!report){findings.append(el('p',tr('No analysis available.','暂无分析。'),'empty'));return;}
  for(const finding of report.findings){const article=el('article','','finding');article.dataset.severity=finding.severity;article.append(el('h3',finding.title),el('p',finding.message));findings.append(article);}
}
function fail(cause:unknown){const code=String(cause);error.hidden=false;error.textContent=code.includes('PERMISSION')?tr('Approve game inspection access to continue.','请批准游戏检查权限后继续。'):code.includes('CHANGED')?tr('Game data changed. Refresh to try again.','游戏数据已更改，请刷新后重试。'):tr('Could not read this game. Try refreshing.','无法读取此游戏，请刷新重试。');}
async function load(){
  if(!game.value||loading)return;const current=++epoch;
  if(checking){checking=false;await client.report.cancel().catch(()=>{});if(current!==epoch)return;}
  loading=true;report=null;error.hidden=true;status.textContent=tr('Reading Mods…','正在读取模组…');ui();
  try{
    const snapshot=await client.inspection.snapshot(game.value,'mods');if(current!==epoch)return;
    const data=parseInventory(snapshot.data);graph=dependencyGraph(data.items);seeds=[];budget=40;listLimit=100;render();
    notice.hidden=!data.truncated&&data.available;notice.textContent=data.truncated?tr('Inventory is incomplete. Unmatched IDs may be outside this snapshot.','清单不完整，未匹配的 ID 可能在快照范围之外。'):tr('Some Mod information is unavailable.','部分模组信息不可用。');
    loading=false;checking=true;status.textContent='';ui();renderReport();
    const value=await client.inspection.check(game.value,'mods',snapshot.revision);if(current!==epoch)return;report=value;checking=false;renderReport();
  }catch(cause){if(current===epoch){checking=false;fail(cause);}}finally{if(current===epoch){loading=false;status.textContent='';ui();}}
}
refresh.onclick=()=>void load();game.onchange=()=>{graph=dependencyGraph([]);selection=null;seeds=[];render();void load();};
search.oninput=()=>{seeds=[];listLimit=100;render();};filter.onchange=()=>{seeds=[];listLimit=100;selection=null;render();};
expand.onclick=()=>{budget=Math.min(160,budget+40);render();};reset.onclick=()=>{budget=40;render();};more.onclick=()=>{listLimit+=100;render();};
cancel.onclick=()=>{epoch++;loading=false;checking=false;void client.report.cancel().catch(fail);status.textContent=tr('Cancelled. Refresh to try again.','已取消，刷新后可重试。');renderReport();ui();};
document.addEventListener('keydown',event=>{if((event.ctrlKey||event.metaKey)&&event.key==='f'){event.preventDefault();search.focus();}});
window.addEventListener('pagehide',()=>{epoch++;if(checking)void client?.report.cancel().catch(()=>{});},{once:true});
void(async()=>{try{client=connect();context=await hostContext(client.context());zh=context.locale==='zh-CN';document.documentElement.lang=zh?'zh-CN':'en';const dark=matchMedia('(prefers-color-scheme: dark)');const theme=()=>document.documentElement.dataset.theme=context.theme==='system'?(dark.matches?'dark':'light'):context.theme;theme();dark.addEventListener('change',theme);labels();const games=context.inspectionGames??context.games;game.replaceChildren(new Option(tr('Choose game','选择游戏'),''),...games.map(item=>new Option(`${item.name} · ${item.version}`,item.id)));game.value=context.instanceId??games[0]?.id??'';render();if(game.value)await load();else status.textContent=tr('No games available.','暂无游戏。');}catch(cause){fail(cause);ui();}})();

installPluginPresentation(presentation);
