// SPDX-License-Identifier: MIT
// Synthetic transport exercises the complete UI. Native copy evidence is separate.
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {mkdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const require=createRequire(new URL('../../../apps/desktop/package.json',import.meta.url));
const {chromium}=require('@playwright/test'),{createServer}=await import('../../../apps/desktop/node_modules/vite/dist/node/index.js');
const root=fileURLToPath(new URL('../../../',import.meta.url)),output=path.join(root,'target/ui-evidence/crash-experiments');mkdirSync(output,{recursive:true});
const server=await createServer({configFile:fileURLToPath(new URL('./vite.config.ts',import.meta.url)),server:{host:'127.0.0.1',port:0},logLevel:'error'});await server.listen();
const address=server.httpServer.address();let browser;const proof={syntheticTransport:true,realMinecraftLaunched:false,checks:[]};
try{
 browser=await chromium.launch({headless:true,channel:'msedge'});
 for(const narrow of [false,true]){
  const page=await browser.newPage({viewport:{width:narrow?560:1250,height:1000},reducedMotion:'reduce'}),errors=[];page.on('pageerror',error=>errors.push(String(error)));
  await page.addInitScript(({zh})=>{
   window.__calls=[];window.__mode='normal';window.__task=0;window.__round=0;
   const runtime={javaMode:'automatic',selectedJavaMajor:21,memoryMb:4096,jvmArgumentCount:0};
   const mod=(id,deps=[])=>({id:`${id}.jar`,name:id==='alpha'?'Adventure Atlas':id==='beta'?'Atlas Library':'Separate Utility',version:'1.0',loader:'fabric',disabled:false,declaredIds:[id],dependencies:deps.map(id=>({id,version:'*',optional:false}))});
   const mods=[mod('alpha',['beta']),mod('beta',['alpha']),mod('separate')];
   const snapshot=()=>({schemaVersion:2,gameId:'synthetic',revision:'a'.repeat(64),baselineAvailable:true,lastCleanLaunch:{startedAt:1700000000000,endedAt:1700000010000,runtime},runtime,launchState:'idle',mods:{available:true,truncated:false,items:[]},configs:{available:true,truncated:false,files:[]},comparison:{available:true,reason:null,baselineEndedAt:1700000010000,failureAt:1700000020000,runtime:{changed:false,before:runtime,after:runtime},loader:{changed:false,before:null,after:null},modOperations:[{id:'update',createdAt:1700000015000,status:'applied',statusAtFailure:'unknown',files:[{name:'Adventure Atlas',before:'alpha-old.jar',after:'alpha.jar'}]}],configBackups:[],limited:false}});
   window.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{window.__calls.push({method,params});
    if(method==='context')return {locale:zh?'zh-CN':'en',theme:zh?'dark':'light',appearance:{accent:'blue',density:'compact',scale:100,motion:'reduced'},instanceId:'synthetic',games:[{id:'synthetic',name:zh?'冒险世界':'Adventure world',version:'1.21.1'}]};
    if(method==='diagnostics.list')return [{id:'latest',name:'logs/latest.log',size:120,modifiedAt:1}];
    if(method==='diagnostics.read')return {id:'latest',name:'logs/latest.log',revision:'b'.repeat(64),text:'[12:00:00] alpha.jar failed during resource loading'};
    if(method==='recovery.snapshot')return snapshot();
    if(method==='inspection.snapshot')return {schemaVersion:1,scope:'mods',game:{id:'synthetic',name:zh?'冒险世界':'Adventure world',version:'1.21.1'},data:{available:true,truncated:false,items:mods},revision:'c'.repeat(64)};
    if(method==='data.read')return JSON.parse(localStorage.getItem('experiments')??'{"value":null,"revision":null}');
    if(method==='data.write'){const old=JSON.parse(localStorage.getItem('experiments')??'{"value":null,"revision":null}');if(old.revision!==params.expectedRevision)throw Error('PLUGIN_DATA_CONFLICT');const result={value:params.value,revision:String(Number(old.revision??0)+1)};localStorage.setItem('experiments',JSON.stringify(result));return result;}
    if(method==='recovery.experimentPreview'){if(window.__mode==='stale')throw Error('TRIAL_COPY_STALE');if(window.__mode==='unsupported')throw Error('EXPERIMENT_DEPENDENCY_METADATA_UNAVAILABLE');const selected=params.modIds.includes('separate.jar')?mods.slice(2):mods.slice(0,2);return {planId:'plan',copyGameId:(++window.__round).toString(16).padStart(32,'0'),game:{id:'synthetic',name:'Adventure world',version:'1.21.1'},name:'Adventure world · diagnosis trial',includeWorlds:params.includeWorlds,fileCount:17,totalBytes:1024*1024*60,requiredBytes:1024*1024*76,availableBytes:1024*1024*1024,enoughSpace:true,destinationLabel:'Games',disabledMods:selected.map(({id,name})=>({id,name})),requestedModIds:params.modIds};}
    if(method==='trialCopy.start'){window.__task=0;return {taskId:'copy-task'};}
    if(method==='trialCopy.task')return {taskId:'copy-task',status:++window.__task>1?'succeeded':'running',completedBytes:0,totalBytes:10,error:null,game:window.__task>1?{id:window.__round.toString(16).padStart(32,'0'),name:'Trial',version:'1.21.1'}:null};
    if(method==='recovery.experimentOpen')return {requested:true};
    if(method==='trialCopy.cleanup')return {removed:true};
    throw Error(`Unexpected fixture method: ${method}`);
   }};
  },{zh:narrow});
  const button=label=>page.getByRole('button',{name:label,exact:true});
  const shot=async name=>{assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);const screenshot=`${name}-${narrow?'560-dark-zh':'1250-light-en'}.png`;await page.screenshot({path:path.join(output,screenshot),fullPage:true});proof.checks.push({name,narrow,screenshot,overflow:false});};
  await page.goto(`http://127.0.0.1:${address.port}/index.html`);await page.waitForFunction(()=>!document.querySelector('#check').disabled);
  assert.equal(await page.evaluate(()=>window.__calls.some(call=>call.method==='data.read')),false);
  await page.locator('#experiments-tab').click();await page.waitForSelector('.experiment-groups label');await shot('context');
  await button(narrow?'预览试验':'Review experiment').click();await page.waitForSelector('.experiment-review li');assert.equal(await page.locator('.experiment-review li').count(),2);await shot('review');
  assert.equal(await page.evaluate(()=>window.__calls.findLast(call=>call.method==='recovery.experimentPreview').params.includeWorlds),false);
  await button(narrow?'创建试验…':'Create experiment…').click();await page.waitForSelector('.experiment-history fieldset');await button(narrow?'打开试验游戏':'Open trial game').click();
  await page.locator('.experiment-history select').selectOption('missingDependency');await button(narrow?'保存结果':'Save result').click();await page.waitForFunction(()=>/Inconclusive|此次结果不确定/.test(document.querySelector('.experiment-history').textContent));await shot('dependency-result');
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('experiments')).value);assert.equal(saved.rounds[0].result,'missingDependency');assert.equal(JSON.stringify(saved).includes('failed during resource'),false);
  await page.reload();await page.waitForFunction(()=>!document.querySelector('#check').disabled);await page.locator('#experiments-tab').click();await page.waitForSelector('.experiment-history article');await shot('persisted-result');
  await button(narrow?'删除试验副本…':'Delete trial copy…').click();await page.waitForFunction(()=>/Copy deleted|副本已删除/.test(document.querySelector('.experiment-history').textContent));
  await page.evaluate(()=>window.__mode='unsupported');await button(narrow?'预览试验':'Review experiment').click();await page.waitForFunction(()=>/Complete dependency metadata|无法读取完整依赖信息/.test(document.querySelector('.experiment-status').textContent));await shot('unsupported');
  assert.equal(await page.evaluate(()=>window.__calls.some(call=>/launch/i.test(call.method))),false);assert.deepEqual(errors,[]);await page.close();
 }
 proof.passed=true;writeFileSync(path.join(output,'render-experiments.json'),JSON.stringify(proof,null,2)+'\n');process.stdout.write(`Experiment fixture passed: ${output}\n`);
}finally{await browser?.close();await server.close();}
