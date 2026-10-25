// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import {connect,hostContext,type Client} from '../../../../packages/plugin-web-sdk/src/index';
import {applicable,byId,equipment,type Enchantment} from './catalog.ts';
import {penalty,steps,solveAsync,validate,type Book,type Enchant,type Request,type Result} from './model.ts';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
type Entry={id:string;level:number;work:number};
const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='',className=''):HTMLElementTagNameMap[K]=>{const node=document.createElement(tag);node.textContent=text;if(className)node.className=className;return node;};
export async function mountAnvilPlanner(client:Client,host=document.getElementById('app')!){
 const context=await hostContext(client.context()),zh=context.locale==='zh-CN';document.documentElement.lang=context.locale;
 const tr=(en:string,cn:string)=>zh?cn:en;host.className='workbench anvil-planner';
 const name=(id:string)=>{const e=byId.get(id)!;return zh?e.zh:e.en;};
 const display=(enchant:Enchant)=>`${name(enchant.id)} ${enchant.level}`;
 const label=(text:string,input:HTMLElement)=>{const node=el('label',text);node.append(input);return node;};
 const option=(select:HTMLSelectElement,value:string,text:string)=>{const node=el('option',text);node.value=value;select.append(node);};
 const button=(text:string,action:string)=>{const node=el('button',text);node.type='button';node.dataset.action=action;return node;};
 let books:Entry[]=[{id:'sharpness',level:5,work:0},{id:'unbreaking',level:3,work:0},{id:'mending',level:1,work:0}],existing:Entry[]=[];
 let controller:AbortController|null=null,result:Result|null=null,calculated:Request|null=null;
 const heading=el('header');heading.append(el('h1',tr('Anvil Planner','铁砧规划')));host.append(heading);
 const contextRow=el('div','','context-row'),version=el('select'),item=el('select');
 for(const v of ['1.20.1','1.21.1'])option(version,v,`Java ${v}`);
 const game=context.games.find(game=>game.id===context.instanceId);version.value=['1.20.1','1.21.1'].includes(game?.version??'')?game!.version:'1.21.1';
 for(const [id,names]of Object.entries(equipment))option(item,id,names[zh?1:0]);
 const workSelect=(value:number)=>{const select=el('select');for(let i=0;i<=6;i++)option(select,String(i),String(penalty(i)));select.value=String(value);return select;};
 const work=workSelect(0);work.dataset.field='item-work';version.dataset.field='version';item.dataset.field='item';
 contextRow.append(label(tr('Version','版本'),version),label(tr('Equipment','装备'),item),label(tr('Prior work penalty','已有惩罚'),work));host.append(contextRow);
 if(game&&!['1.20.1','1.21.1'].includes(game.version))host.append(el('p',tr(`Current game: ${game.version}. Choose a supported vanilla version.`,`当前游戏：${game.version}。请选择支持的原版版本。`),'notice'));
 const layout=el('div','','planner-layout'),inputs=el('section','','input-panel'),output=el('section','','output-panel');host.append(layout);layout.append(inputs,output);
 const existingDetails=el('details','','existing-details'),existingSummary=el('summary',tr('Existing enchantments','已有附魔')),existingList=el('div','','entries'),addExisting=button(tr('Add existing enchantment','添加已有附魔'),'add-existing');
 existingDetails.append(existingSummary,existingList,addExisting);inputs.append(existingDetails);
 const booksHeading=el('div','','section-heading'),bookTitle=el('h2'),preset=button(tr('Sword preset','剑预设'),'preset');booksHeading.append(bookTitle,preset);inputs.append(booksHeading);
 const bookList=el('div','','entries'),addBook=button(tr('Add book','添加附魔书'),'add-book');inputs.append(bookList,addBook);
 const actions=el('div','','calculate-actions'),calculate=button(tr('Find best order','规划顺序'),'calculate'),cancel=button(tr('Cancel','取消'),'cancel');calculate.className='primary';cancel.hidden=true;
 const creative=el('input');creative.type='checkbox';creative.dataset.field='creative';const creativeLabel=label(tr('Allow 40+ levels (Creative)','允许 40 级以上（创造模式）'),creative);creativeLabel.className='check-label';actions.append(calculate,cancel,creativeLabel);inputs.append(actions);
 const error=el('p','','error');error.role='alert';error.hidden=true;host.append(error);
 const status=el('p','','status');status.role='status';host.append(status);
 const scope=el('details','','scope');scope.append(el('summary',tr('Rules and supported inputs','规则与支持范围')),el('p',tr('Vanilla Java 1.20.1 / 1.21.1 · listed equipment only. One enchantment per input book, up to 8 books. Existing equipment enchantments and prior work are included.','原版 Java 1.20.1 / 1.21.1，仅支持列表中的装备。每本输入书含一种附魔，最多 8 本。计入装备已有附魔和惩罚。')),el('p',tr('The objective is the smallest sum of level costs, with every survival operation below 40. Equal enchantment levels can merge to the next level. All books are consumed and the highest attainable enchantment levels are kept.','目标是等级消耗之和最低，生存模式每步必须低于 40 级。相同附魔等级可合并升级。使用全部输入书，并保留可达到的最高附魔等级。')),el('p',tr('Prior work penalty: 0, 1, 3, 7, 15, 31, 63. After an enchantment combine, the new penalty is twice the larger input penalty plus 1. Durability repair, renaming costs, mixed-enchantment input books and modified server rules are outside this planner.','已有惩罚：0、1、3、7、15、31、63。附魔合并后的惩罚为两侧较大惩罚的 2 倍加 1。此工具不计算耐久修复、改名费用、多附魔输入书及服务器修改规则。')));host.append(scope);
 function stop(){controller?.abort();controller=null;cancel.hidden=true;calculate.disabled=false;host.setAttribute('aria-busy','false');}
 function invalidate(){stop();result=null;calculated=null;error.hidden=true;status.textContent='';renderResult();}
 function errorMessage(code:string){return ({VERSION_UNSUPPORTED:tr('Choose Java 1.20.1 or 1.21.1.','请选择 Java 1.20.1 或 1.21.1。'),ENCHANT_CONFLICT:tr('These enchantments conflict. Remove one before planning.','附魔不兼容，请移除冲突的附魔后重试。'),EXISTING_DUPLICATE:tr('Enter each existing equipment enchantment once.','每种装备已有附魔只需输入一次。'),BOOK_LIMIT:tr('Add between 1 and 8 single-enchantment books.','请添加 1 至 8 本单附魔书。'),SEARCH_LIMIT:tr('Search limit reached. Try fewer books.','已达到搜索上限，请减少附魔书数量。'),WORK_INVALID:tr('Choose a supported prior work penalty.','请选择支持的已有惩罚。'),LEVEL_INVALID:tr('Choose a valid enchantment level.','请选择有效的附魔等级。'),ENCHANT_UNSUPPORTED:tr('This enchantment does not apply to the selected equipment.','该附魔不适用于所选装备。')})[code]??tr('Could not calculate this plan. Retry with fewer books.','无法计算此计划，请减少附魔书后重试。');}
 function fail(code:string){error.textContent=errorMessage(code);error.hidden=false;}
 function renderRows(list:HTMLElement,entries:Entry[],isBook:boolean){
  list.replaceChildren();entries.forEach((entry,index)=>{
   const row=el('div','','entry');row.dataset.index=String(index);
   const enchant=el('select'),level=el('select'),prior=workSelect(entry.work);
   for(const data of applicable(item.value))option(enchant,data.id,zh?data.zh:data.en);enchant.value=entry.id;
   const setLevels=(data:Enchantment)=>{level.replaceChildren();for(let i=1;i<=data.max;i++)option(level,String(i),String(i));level.value=String(Math.min(data.max,entry.level));};setLevels(byId.get(entry.id)!);
   enchant.dataset.field='enchantment';level.dataset.field='level';prior.dataset.field='book-work';
   const remove=button(tr('Remove','移除'),'remove');remove.setAttribute('aria-label',tr(`Remove ${isBook?'book':'existing enchantment'} ${index+1}`,`移除第 ${index+1} ${isBook?'本书':'项已有附魔'}`));
   const number=el('strong',isBook?tr(`Book ${index+1}`,`书 ${index+1}`):tr(`${index+1}`,`${index+1}`),'entry-number');
   row.append(number,label(tr('Enchantment','附魔'),enchant),label(tr('Level','等级'),level));if(isBook)row.append(label(tr('Prior work penalty','已有惩罚'),prior));row.append(remove);
   enchant.onchange=()=>{entry.id=enchant.value;entry.level=byId.get(entry.id)!.max;setLevels(byId.get(entry.id)!);invalidate();};
   level.onchange=()=>{entry.level=Number(level.value);invalidate();};prior.onchange=()=>{entry.work=Number(prior.value);invalidate();};
   remove.onclick=()=>{entries.splice(index,1);invalidate();renderInputs();};list.append(row);
  });
 }
 function renderInputs(){renderRows(bookList,books,true);renderRows(existingList,existing,false);bookTitle.textContent=tr(`Books (${books.length}/8)`,`附魔书（${books.length}/8）`);addBook.disabled=books.length>=8;addExisting.disabled=existing.length>=16;existingSummary.textContent=tr(`Existing enchantments (${existing.length})`,`已有附魔（${existing.length}）`);}
 const reference=(value:string,request:Request)=>value==='item'?equipment[request.item]![zh?1:0]:value.startsWith('book:')?tr(`Book ${value.slice(5)} · ${display(request.books[Number(value.slice(5))-1]!)}`,`书 ${value.slice(5)} · ${display(request.books[Number(value.slice(5))-1]!)}`):tr(`Result of step ${value.slice(5)}`,`第 ${value.slice(5)} 步结果`);
 function planText(){if(!result||!calculated)return '';return [`${tr('Anvil Planner','铁砧规划')} · Java ${calculated.version} · ${equipment[calculated.item]![zh?1:0]}`,`${tr('Total levels','等级合计')}: ${result.root.total} · ${tr('Peak','单步最高')}: ${result.root.peak}`,`${tr('Equipment prior work penalty','装备已有惩罚')}: ${penalty(calculated.work)}`,`${tr('Existing enchantments','已有附魔')}: ${calculated.existing.map(display).join(', ')||'—'}`,...calculated.books.map((book,i)=>`${tr('Book','书')} ${i+1}: ${display(book)} (${tr('penalty','惩罚')} ${penalty(book.work)})`),...(!result.survival?[tr('Too expensive for Survival: one or more operations cost 40+ levels.','生存模式过于昂贵：至少一步需要 40 级以上。')]:[]),...steps(result.root).map(step=>`${step.id}. ${tr('Left','左槽')}: ${reference(step.left,calculated!)} + ${tr('Right','右槽')}: ${reference(step.right,calculated!)} → ${step.cost} ${tr('levels','级')} (${tr('result penalty','结果惩罚')} ${step.penalty})`)].join('\n');}
 function renderResult(){
  output.replaceChildren();if(!result||!calculated){output.append(el('p',tr('Add your books, then find the best order.','填好附魔书后，规划合并顺序。'),'empty'));return;}
  if(!result.survival)output.append(el('p',calculated.creative?tr('Creative only · at least one operation costs 40+ levels.','仅适用于创造模式，至少一步需要 40 级以上。'):tr('Too expensive for Survival. This is the cheapest Creative order.','生存模式过于昂贵。以下为消耗最低的创造模式顺序。'),'notice'));
  const stats=el('div','','stats');for(const [value,text]of [[result.root.total,tr('Total levels','等级合计')],[result.root.peak,tr('Highest step','单步最高')],[penalty(result.root.work),tr('Final penalty','最终惩罚')]] as const){const metric=el('div','','metric');metric.append(el('strong',String(value)),el('span',text));stats.append(metric);}output.append(stats);
  const final=el('p',result.root.enchants.map(display).join(' · '),'final-enchants');output.append(final);
  const list=el('ol','','steps');for(const step of steps(result.root)){
   const row=el('li');if(step.cost>=40)row.dataset.expensive='true';
   const top=el('div','','step-heading');top.append(el('strong',tr(`Step ${step.id}`,`第 ${step.id} 步`)),el('strong',tr(`${step.cost} levels`,`${step.cost} 级`),'cost'));row.append(top);
   const slots=el('div','','slots');slots.append(label(tr('Left slot','左槽'),el('span',reference(step.left,calculated))),label(tr('Right slot','右槽'),el('span',reference(step.right,calculated))));row.append(slots,el('p',tr(`Result penalty ${step.penalty}`,`结果惩罚 ${step.penalty}`),'muted'));list.append(row);
  }output.append(list);
  const copy=button(tr('Copy plan','复制计划'),'copy');copy.onclick=async()=>{try{await client.clipboard.writeText(planText());status.textContent=tr('Plan copied.','计划已复制。');}catch{status.textContent=tr('Could not copy the plan. Check clipboard permission and retry.','无法复制计划，请检查剪贴板权限后重试。');}};output.append(copy);
 }
 function add(entries:Entry[]){const available=applicable(item.value),selected=available.find(data=>!entries.some(e=>e.id===data.id))??available[0]!;entries.push({id:selected.id,level:selected.max,work:0});invalidate();renderInputs();}
 addBook.onclick=()=>{if(books.length<8)add(books);};addExisting.onclick=()=>{if(existing.length<16){add(existing);existingDetails.open=true;}};
 preset.onclick=()=>{item.value='sword';work.value='0';existing=[];books=[{id:'sharpness',level:5,work:0},{id:'unbreaking',level:3,work:0},{id:'mending',level:1,work:0}];invalidate();renderInputs();};
 item.onchange=()=>{const ids=new Set(applicable(item.value).map(data=>data.id));books=books.filter(entry=>ids.has(entry.id));existing=existing.filter(entry=>ids.has(entry.id));invalidate();renderInputs();};
 version.onchange=invalidate;work.onchange=invalidate;creative.onchange=invalidate;
 calculate.onclick=async()=>{
  invalidate();const request:Request={version:version.value,item:item.value,work:Number(work.value),existing:existing.map(({id,level})=>({id,level})),books:structuredClone(books) as Book[],creative:creative.checked};
  const active=new AbortController();controller=active;
  try{validate(request);host.setAttribute('aria-busy','true');calculate.disabled=true;cancel.hidden=false;status.textContent=tr('Finding the lowest level cost…','正在寻找等级消耗最低的顺序…');
   const answer=await solveAsync(request,active.signal);if(controller!==active)return;stop();status.textContent='';result=answer;calculated=request;renderResult();
  }catch(cause){if(controller!==active)return;stop();status.textContent='';fail(cause instanceof Error?cause.message:'CALCULATION_FAILED');}
 };
 cancel.onclick=()=>{stop();status.textContent=tr('Calculation cancelled.','计算已取消。');};window.addEventListener('pagehide',stop,{once:true});
 renderInputs();renderResult();host.setAttribute('aria-busy','false');return {stop};
}
if(document.getElementById('app'))void mountAnvilPlanner(connect()).catch(()=>{document.getElementById('app')!.textContent='Could not open Anvil Planner / 无法打开铁砧规划';});

installPluginPresentation(presentation);
