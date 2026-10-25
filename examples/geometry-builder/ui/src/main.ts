// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import {connect,hostContext,type Client} from '../../../../packages/plugin-web-sdk/src/index';
import {generate,materials,occupancy,type Result,type Shape} from './model.ts';
import {drawLayer} from './canvas.ts';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';
const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='')=>{const n=document.createElement(tag);n.textContent=text;return n;};
export async function mountGeometryBuilder(client:Client,host=document.getElementById('app')!){
 const context=await hostContext(client.context()),zh=context.locale==='zh-CN',tr=(en:string,cn:string)=>zh?cn:en;
 document.documentElement.lang=context.locale;host.className='workbench geometry-builder';
 const field=(text:string,node:HTMLElement)=>{const label=el('label',text);label.append(node);return label;};
 const option=(select:HTMLSelectElement,value:string,text:string)=>{const n=el('option',text);n.value=value;select.append(n);};
 const shape=el('select');for(const [v,en,cn] of [['circle','Circle','圆'],['ellipse','Ellipse','椭圆'],['sphere','Sphere','球'],['dome','Dome','半球顶']])option(shape,v!,tr(en!,cn!));
 shape.dataset.field='shape';const width=el('input'),depth=el('input');for(const n of [width,depth]){n.type='number';n.min='1';n.max='128';n.step='1';n.value='25';}width.dataset.field='width';depth.dataset.field='depth';
 const material=el('select');const names=[['Stone','石头'],['Cobblestone','圆石'],['Glass','玻璃'],['Oak planks','橡木木板'],['White concrete','白色混凝土']];materials.forEach((m,i)=>option(material,m,tr(...names[i] as [string,string])));
 const hollow=el('input');hollow.type='checkbox';hollow.checked=true;
 const hollowField=el('label');hollowField.className='check-field';hollowField.append(hollow,document.createTextNode(tr('Hollow','空心')));
 const controls=el('div');controls.className='controls';const depthField=field(tr('Depth','深度'),depth);controls.append(field(tr('Shape','形状'),shape),field(tr('Width / diameter (1–128)','宽度 / 直径（1–128）'),width),depthField,field(tr('Material','材料'),material),hollowField);
 const calculate=el('button',tr('Generate','生成')),cancel=el('button',tr('Cancel','取消')),save=el('button',tr('Save to blueprints','保存到蓝图库'));calculate.dataset.action='generate';cancel.hidden=true;save.disabled=true;save.dataset.action='save';
 calculate.className='primary';const open=el('button',tr('Open blueprints','打开蓝图库'));open.hidden=true;open.dataset.action='open';
 const actions=el('div');actions.className='actions';actions.append(calculate,cancel,save,open);
 const status=el('p');status.setAttribute('role','status');const panel=el('section');panel.className='geometry-result';panel.hidden=true;
 const count=el('p'),center=el('p'),canvas=el('canvas'),layer=el('input'),layerLabel=el('output');count.className='result-count';center.className='center-note';layer.type='range';layer.min='0';layer.step='1';layer.setAttribute('aria-label',tr('Layer','层'));canvas.setAttribute('role','img');
 const nav=el('div');nav.className='layer-nav';const previous=el('button',tr('Previous','上一层')),next=el('button',tr('Next','下一层'));nav.append(previous,layer, next,layerLabel);panel.append(count,nav,canvas,center);
 host.replaceChildren(el('h1',tr('Geometry Builder','几何建筑')),controls,actions,status,panel);
 let active:AbortController|undefined,result:Result|undefined,savedId:string|undefined;
 const render=()=>{if(!result)return;const i=Number(layer.value),n=result.layers[i]!.length;drawLayer(canvas,result,i);layerLabel.textContent=tr(`Layer ${i+1} / ${result.layers.length} · ${n.toLocaleString()} blocks`,`第 ${i+1} / ${result.layers.length} 层 · ${n.toLocaleString()} 个方块`);canvas.setAttribute('aria-label',layerLabel.textContent);previous.disabled=i===0;next.disabled=i===result.layers.length-1;};
 const stop=()=>{active?.abort();active=undefined;cancel.hidden=true;calculate.disabled=false;host.setAttribute('aria-busy','false');};
 const invalidate=()=>{stop();result=undefined;savedId=undefined;open.hidden=true;panel.hidden=true;save.disabled=true;status.textContent='';depthField.hidden=shape.value!=='ellipse';};
 for(const n of [shape,width,depth,material,hollow])n.onchange=invalidate;
 calculate.onclick=async()=>{invalidate();const controller=new AbortController();active=controller;calculate.disabled=true;cancel.hidden=false;host.setAttribute('aria-busy','true');status.textContent=tr('Generating…','正在生成…');try{
  const answer=await generate({shape:shape.value as Shape,width:Number(width.value),depth:shape.value==='ellipse'?Number(depth.value):Number(width.value),material:material.value,hollow:hollow.checked},controller.signal);if(active!==controller)return;stop();result=answer;panel.hidden=false;save.disabled=false;layer.max=String(answer.layers.length-1);layer.value='0';layer.hidden=answer.layers.length===1;previous.hidden=next.hidden=answer.layers.length===1;
  count.textContent=tr(`${answer.blocks.length.toLocaleString()} blocks · ${answer.dimensions.x} × ${answer.dimensions.y} × ${answer.dimensions.z}`,`${answer.blocks.length.toLocaleString()} 个方块 · ${answer.dimensions.x} × ${answer.dimensions.y} × ${answer.dimensions.z}`);
  const cx=(answer.dimensions.x-1)/2,cz=(answer.dimensions.z-1)/2;center.textContent=tr(`Centre X ${cx}, Z ${cz} · dashed lines mark the centre. Integer centres pass through a block; half centres lie between blocks.`,`中心 X ${cx}，Z ${cz} · 虚线表示中心。整数位于方块中心，半整数位于方块之间。`);status.textContent='';render();
 }catch(error){if(active!==controller)return;stop();status.textContent=String(error).includes('VOLUME_LIMIT')?tr('This shape exceeds the blueprint volume limit. Use a smaller diameter (sphere: 101 maximum).','形状超过蓝图体积上限，请缩小直径（球体最大 101）。'):String(error).includes('BLOCK_LIMIT')?tr('Too many blocks (limit 262,144). Use a smaller size or hollow shape.','方块超过 262,144 个，请缩小尺寸或选择空心。'):tr('Use whole-number dimensions from 1 to 128.','尺寸须为 1–128 的整数。');}};
 cancel.onclick=()=>{stop();status.textContent=tr('Generation cancelled.','已取消生成。');};layer.oninput=render;previous.onclick=()=>{layer.value=String(Number(layer.value)-1);render();};next.onclick=()=>{layer.value=String(Number(layer.value)+1);render();};
 open.onclick=async()=>{if(!savedId)return;open.disabled=true;try{await client.generatedBlueprint.open(savedId);}catch{status.textContent=tr('Blueprint saved. Could not open the library; try Open blueprints again.','蓝图已保存。未能打开蓝图库，可再次点击打开。');}finally{open.disabled=false;}};
 save.onclick=async()=>{if(!result||savedId)return;const snapshot=result;save.disabled=true;try{const saved=await saveBlueprint(client,snapshot);if(result!==snapshot)return;savedId=saved.blueprintId;open.hidden=false;status.textContent=tr('Saved to blueprints.','已保存到蓝图库。');open.click();}catch{if(result===snapshot)status.textContent=tr('Could not save the blueprint. Retry.','无法保存蓝图，请重试。');}finally{save.disabled=!result||!!savedId;}};
 window.addEventListener('pagehide',stop,{once:true});invalidate();return {stop};
}
// The host owns validation and library writes. No filesystem paths cross this boundary.
async function saveBlueprint(client:Client,result:Result){return client.generatedBlueprint.save({name:result.name,width:result.dimensions.x,height:result.dimensions.y,length:result.dimensions.z,block:result.palette[0]!,occupancy:occupancy(result)});}
if(document.getElementById('app'))void mountGeometryBuilder(connect()).catch(()=>{document.getElementById('app')!.textContent='Could not open Geometry Builder / 无法打开几何建筑';});

installPluginPresentation(presentation);
