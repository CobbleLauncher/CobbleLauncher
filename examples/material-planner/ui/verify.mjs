// SPDX-License-Identifier: MIT
// Source-rendered UI with synthetic SDK host; native acceptance is separate.
import {createRequire} from 'node:module';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {mkdir,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const require=createRequire(resolve('apps/desktop/package.json'));
const {createServer}=await import(pathToFileURL(require.resolve('vite')).href);
const {chromium}=require('@playwright/test');
const server=await createServer({configFile:false,root:resolve('examples/material-planner/ui'),server:{host:'127.0.0.1',port:0,cors:{origin:'*'}},clearScreen:false});
await server.listen();
const out=resolve('examples/material-planner/validation');await mkdir(out,{recursive:true});
const browser=await chromium.launch({channel:'msedge',headless:true});
const receipts=[];
async function fixture(locale='en',theme='light',width=1280,scenario='normal',count=12){
 const page=await browser.newPage({viewport:{width,height:940}});
 await page.route('https://**/*',route=>route.abort());
 await page.addInitScript(({locale,theme,scenario,count})=>{
  const docs=new Map();let rev=0;
  globalThis.calls=[];globalThis.dirty=[];
  globalThis.externalChange=()=>{for(const[key,value]of docs)if(key.includes('p-'))docs.set(key,{...value,revision:'external'});};
  globalThis.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{
   calls.push({method,params:structuredClone(params??{})});
   if(method==='context')return{locale,theme,instanceId:null,games:[{id:'game-a',name:'Create Together',version:'1.20.1'}]};
   if(method==='handoffs.take')return scenario==='empty'||scenario==='error'?null:{handoffId:'fixture',sender:{pluginId:'blueprint',name:'Blueprint Library'},game:null,payload:{name:locale==='zh-CN'?'河岸小屋':'Riverside cottage',source:locale==='zh-CN'?'河岸小屋.litematic':'riverside-cottage.litematic',materials:Array.from({length:count},(_,i)=>({id:`minecraft:${['oak_planks','stone_bricks','glass','oak_stairs','lantern','oak_fence'][i%6]}_${i}`,name:(locale==='zh-CN'?['橡木木板','石砖','玻璃','橡木楼梯','灯笼','橡木栅栏']:['Oak Planks','Stone Bricks','Glass','Oak Stairs','Lantern','Oak Fence'])[i%6],required:Math.max(1,384-i*17),stackSize:null}))}};
   if(method==='view.setDirty'){dirty.push(params.dirty);return null;}
   if(method==='clipboard.writeText')return null;
   if(method==='exports.text')return{saved:false};
   if(method==='inspection.snapshot')return{data:{items:[{folder_name:'WorldA',name:'World A'}]}};
   const key=JSON.stringify(params.scope)+params.key;
   if(method==='data.read'){if(scenario==='error')throw Error('PLUGIN_STORAGE_UNAVAILABLE');return structuredClone(docs.get(key)??{value:null,revision:null});}
   if(method==='data.write'||method==='data.remove'){
    if((docs.get(key)?.revision??null)!==params.expectedRevision)throw Error('PLUGIN_DATA_CONFLICT');
    if(method==='data.remove'){docs.delete(key);return null;}
    const doc={value:structuredClone(params.value),revision:`r${++rev}`};docs.set(key,doc);return structuredClone(doc);
   }
   throw Error(`UNEXPECTED:${method}`);
  }};
 },{locale,theme,scenario,count});
 await page.goto(server.resolvedUrls.local[0]);
 await page.waitForFunction(()=>document.querySelector('.material-planner')?.getAttribute('aria-busy')==='false');
 return page;
}
try{
 for(const sample of [{locale:'en',theme:'light',width:1280},{locale:'zh-CN',theme:'dark',width:560}]){
  const page=await fixture(sample.locale,sample.theme,sample.width);
  assert.equal(await page.locator('tbody tr').count(),12);
  assert.equal(await page.locator('[data-field=game]').inputValue(),'');
  assert.equal(await page.evaluate(()=>calls.some(c=>c.method==='data.write')),false);
  await page.locator('tbody [data-field=collected]').first().fill('128');
  await page.locator('tbody [data-field=stack-size]').first().fill('64');
  await page.screenshot({path:resolve(out,`material-planner-${sample.locale}-${sample.width}.png`),fullPage:true});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
  await page.locator('[data-action=save]').click();await page.waitForFunction(()=>dirty.at(-1)===false);
  assert.deepEqual(await page.evaluate(()=>calls.filter(c=>c.method==='data.write').at(-1).params.scope),{});
  await page.locator('tbody [data-field=collected]').first().fill('129');await page.evaluate(()=>externalChange());
  await page.locator('[data-action=save]').click();await page.locator('.error').waitFor({state:'visible'});
  assert.equal(await page.locator('tbody [data-field=collected]').first().inputValue(),'129');
  await page.screenshot({path:resolve(out,`material-planner-conflict-${sample.locale}-${sample.width}.png`),fullPage:true});
  await page.locator('[data-action=reload]').click();await page.waitForFunction(()=>document.querySelector('.material-planner').getAttribute('aria-busy')==='false');
  assert.equal(await page.locator('tbody [data-field=collected]').first().inputValue(),'129');
  await page.locator('[data-action=save]').click();await page.waitForFunction(()=>dirty.at(-1)===false);
  await page.locator('[data-field=search]').fill('not-present');await page.locator('.empty').filter({hasText:sample.locale==='en'?'No matching':'没有匹配'}).waitFor({state:'visible'});
  await page.screenshot({path:resolve(out,`material-planner-search-${sample.locale}-${sample.width}.png`),fullPage:true});
  receipts.push({...sample,globalScope:true,unsavedPrefill:true,progressSave:true,conflictDraftPreserved:true,reloadRetry:true,noHorizontalPageOverflow:true});await page.close();
 }
 for(const scenario of ['empty','error']){
  const page=await fixture('zh-CN','dark',560,scenario);
  await page.screenshot({path:resolve(out,`material-planner-${scenario}-zh-CN-560.png`),fullPage:true});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);await page.close();
 }
 const large=await fixture('en','light',1280,'normal',1000);
 assert.equal(await large.locator('tbody tr').count(),1000);
 await large.locator('[data-action=save]').click();await large.waitForFunction(()=>dirty.at(-1)===false);
 const lastId=await large.locator('tbody tr').nth(999).getAttribute('data-material');
 await large.locator('tbody [data-field=collected]').nth(999).fill('55');
 await large.locator('[data-action=save]').click();await large.waitForFunction(()=>dirty.at(-1)===false);
 await large.locator('[data-field=plan]').selectOption('');await large.waitForFunction(()=>document.querySelector('.plan-editor').hidden);
 const saved=await large.locator('[data-field=plan] option').nth(1).getAttribute('value');
 await large.locator('[data-field=plan]').selectOption(saved);await large.locator('tbody tr').nth(999).waitFor();
 assert.equal(await large.locator(`tr[data-material="${lastId}"] [data-field=collected]`).inputValue(),'55');
 await large.locator('[data-action=copy]').click();assert.equal(await large.evaluate(()=>calls.some(c=>c.method==='clipboard.writeText')),false);
 await large.locator('[data-action=export]').click();assert.equal(await large.evaluate(()=>calls.filter(c=>c.method==='exports.text').at(-1).params.text.split('\n').length),1005);
 receipts.push({rows:1000,saveReopenProgress:true,noClipboardTruncation:true,fullExportCancelledWithoutSuccess:true});await large.close();
 await writeFile(resolve(out,'summary.json'),JSON.stringify({evidence:'source-rendered UI with synthetic SDK host; no native storage/clipboard claim',receipts},null,2));
 console.log(JSON.stringify({pass:receipts.length,evidence:resolve(out,'summary.json')}));
}finally{await browser.close();await server.close();}
