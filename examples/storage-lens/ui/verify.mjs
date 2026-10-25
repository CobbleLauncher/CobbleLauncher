// SPDX-License-Identifier: MIT
// Source-rendered UI, synthetic host storage/picker/tasks. Not a native filesystem acceptance.
import {createRequire} from 'node:module';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {mkdir,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const require=createRequire(resolve('apps/desktop/package.json'));
const {createServer}=await import(pathToFileURL(require.resolve('vite')).href);
const {chromium}=require('@playwright/test');
const server=await createServer({configFile:false,root:resolve('examples/storage-lens/ui'),server:{host:'127.0.0.1',port:0,cors:{origin:'*'}},clearScreen:false});
await server.listen();const out=resolve('examples/storage-lens/validation');await mkdir(out,{recursive:true});
const browser=await chromium.launch({channel:'msedge',headless:true});const receipts=[];
async function fixture(locale='en',theme='light',width=1280,scenario='normal'){
 const page=await browser.newPage({viewport:{width,height:850}});
 await page.route('https://**/*',route=>route.abort());
 await page.addInitScript(({locale,theme,scenario})=>{
  globalThis.calls=[];let taskReads=0,cancelled=false,removed=false;
  globalThis.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{
   calls.push({method,params:structuredClone(params??{})});
   if(method==='context')return{locale,theme,instanceId:null,games:[]};
   if(method==='view.setDirty')return null;
   if(method==='storageWorkspace.overview'){
    if(scenario==='error')throw Error('STORAGE_IO_FAILED');
    return{games:scenario==='empty'?[]:[{gameId:'game-a',name:locale==='zh-CN'?'山间整合包':'Mountain modpack',instanceBytes:14542610432,backupBytes:3532554240,movable:true},{gameId:'game-b',name:locale==='zh-CN'?'原版生存':'Vanilla survival',instanceBytes:3542610432,backupBytes:82554240,activity:"running",movable:false},{gameId:'game-c',name:locale==='zh-CN'?'离线磁盘':'Offline disk',instanceBytes:null,backupBytes:0,movable:false}],cache:{bytes:43826688,reclaimableBytes:43826688},retained:removed?[]:[{taskId:'receipt123',gameId:'game-a',bytes:1024000000,sourceLabel:'old-game',canRecycle:true}]};
   }
   if(method==='storageWorkspace.pickDestination')return{planId:'plan123',targetLabel:locale==='zh-CN'?'游戏磁盘':'Games drive',requiredBytes:14542610432,availableBytes:284542610432,enoughSpace:true,sourceRetained:true};
   if(method==='storageWorkspace.startMove')return{taskId:'task123'};
   if(method==='storageWorkspace.task'){
    taskReads++;let status=cancelled?'cancelled':taskReads>2?(scenario==='failure'?'failed':'succeeded'):'running';
    return{taskId:'task123',status,phase:status==='running'?'copying':'complete',completedBytes:taskReads>2?14542610432:6500000000,totalBytes:14542610432,error:status==='failed'?'STORAGE_VERIFY_FAILED':status==='cancelled'?'STORAGE_CANCELLED':null,sourceRetained:true};
   }
   if(method==='storageWorkspace.cancel'){cancelled=true;return null;}
   if(method==='storageWorkspace.cleanCache')return{cleared:true,reclaimedBytes:43826688};
   if(method==='storageWorkspace.releaseSource'){removed=true;return{released:true,recycledBytes:1024000000};}
   if(method==='storageWorkspace.openSource')return null;
   throw Error(`UNEXPECTED:${method}`);
  }};
 },{locale,theme,scenario});
 await page.goto(server.resolvedUrls.local[0]);await page.waitForFunction(()=>document.querySelector('.storage-workspace')?.getAttribute('aria-busy')==='false');return page;
}
try{
 for(const sample of [{locale:'en',theme:'light',width:1280},{locale:'zh-CN',theme:'dark',width:560}]){
  const page=await fixture(sample.locale,sample.theme,sample.width);
  assert.equal(await page.locator('tbody tr').count(),3);
  await page.screenshot({path:resolve(out,`storage-${sample.locale}-${sample.width}.png`),fullPage:true});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
  await page.locator('[data-action=pick-destination]').first().click();
  await page.locator('.move-preview').waitFor({state:'visible'});
  assert.equal(await page.evaluate(()=>calls.some(c=>c.method==='storageWorkspace.startMove')),false);
  await page.screenshot({path:resolve(out,`storage-preview-${sample.locale}-${sample.width}.png`),fullPage:true});
  await page.locator('[data-action=start-move]').click();
  await page.waitForFunction(()=>calls.filter(c=>c.method==='storageWorkspace.task').length>=3);
  await page.locator('[data-action=release-source]').click();await page.waitForFunction(()=>!document.querySelector('[data-action=release-source]'));
  assert.equal(await page.evaluate(()=>calls.some(c=>c.method==='storageWorkspace.releaseSource')),true);
  receipts.push({...sample,previewBeforeWrite:true,taskPoll:true,explicitSourceRecycle:true,noPageOverflow:true});await page.close();
 }
 const failure=await fixture('zh-CN','dark',560,'failure');await failure.locator('[data-action=pick-destination]').first().click();await failure.locator('[data-action=start-move]').click();await failure.waitForFunction(()=>calls.filter(c=>c.method==='storageWorkspace.task').length>=3);await failure.screenshot({path:resolve(out,'storage-failure-zh-CN-560.png'),fullPage:true});assert.match(await failure.locator('.move-task').textContent(),/原游戏仍保留/);receipts.push({failedTask:true,originalKept:true});await failure.close();
 const cancelled=await fixture();await cancelled.locator('[data-action=pick-destination]').first().click();await cancelled.locator('[data-action=start-move]').click();await cancelled.locator('[data-action=cancel-move]').click();await cancelled.waitForFunction(()=>document.querySelector('.move-task').textContent.includes('cancelled'));receipts.push({cancelRequest:true,originalKept:true});await cancelled.close();
 for(const scenario of ['empty','error']){const page=await fixture('zh-CN','dark',560,scenario);await page.screenshot({path:resolve(out,`storage-${scenario}-zh-CN-560.png`),fullPage:true});await page.close();}
 await writeFile(resolve(out,'summary.json'),JSON.stringify({evidence:'synthetic host, actual source-rendered UI; native filesystem not exercised',receipts},null,2));console.log(JSON.stringify({pass:receipts.length,evidence:resolve(out,'summary.json')}));
}finally{await browser.close();await server.close();}
