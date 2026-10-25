// SPDX-License-Identifier: MIT
// Synthetic transport verifies the compiled UI, not native IPC or Wasm execution.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const require=createRequire(new URL('../../../apps/desktop/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const output=path.join(root,'target/ui-evidence/modpack-inspector');mkdirSync(output,{recursive:true});
const mod=(id,extra={})=>({id:`${id}.jar`,name:id,version:'1.2.0',loader:'fabric',disabled:false,declaredIds:[id],dependencies:[],...extra});
const items=[
 mod('Adventure Atlas',{dependencies:[{id:'world-api',version:'>=3.0',optional:false},{id:'missing-library',version:'~2.0',optional:false},{id:'java',version:'>=21',optional:false},{id:'off-library',version:'*',optional:false}]}),
 mod('World API',{declaredIds:['world-api'],version:'3.1.0'}),
 mod('Disabled Library',{declaredIds:['off-library'],disabled:true}),
 mod('Metadata unavailable',{declaredIds:[],version:null}),
 ...Array.from({length:126},(_,i)=>mod(`Utility ${String(i).padStart(3,'0')}`,{dependencies:i<125?[{id:`Utility ${String(i+1).padStart(3,'0')}`,version:'*',optional:false}]:[]})),
];
const snapshot={schemaVersion:1,scope:'mods',game:{id:'synthetic-game',name:'Synthetic dependency fixture',version:'1.21.1'},data:{items,available:true,truncated:false},revision:'a'.repeat(64)};
const report={schema_version:1,findings:[{code:'fixture-only',severity:'warning',title:'Synthetic analysis',message:'Inventory matching is by declared ID. Version ranges and compatibility remain unchecked.',evidence:[],suggestions:[]}],tables:[]};
const server=createServer((request,response)=>{try{const local=path.resolve(root,'examples/modpack-inspector/ui/dist',decodeURIComponent((request.url??'/').split('?')[0]).slice(1));const base=path.resolve(root,'examples/modpack-inspector/ui/dist');if(!local.startsWith(base+path.sep))throw Error('Invalid route');response.setHeader('Content-Type',local.endsWith('.js')?'text/javascript':local.endsWith('.css')?'text/css':local.endsWith('.json')?'application/json':'text/html');response.end(readFileSync(local));}catch{response.writeHead(404);response.end();}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;const result={syntheticTransport:true,syntheticInventory:true,nativeIpcTested:false,checks:[]};
try{
 browser=await chromium.launch({headless:true,channel:'msedge'});
 for(const narrow of [false,true]){
  const page=await browser.newPage({viewport:narrow?{width:540,height:920}:{width:1440,height:980}}),errors=[];page.on('pageerror',error=>errors.push(String(error)));
  await page.addInitScript(({snapshot,report,narrow})=>{window.__calls=[];window.__mode='normal';window.__pending=null;window.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{window.__calls.push({method,params});if(method==='context')return{locale:narrow?'zh-CN':'en',theme:narrow?'dark':'light',instanceId:snapshot.game.id,games:[snapshot.game],inspectionGames:[snapshot.game]};if(method==='inspection.snapshot'){if(window.__mode==='error')throw Error('INSPECTION_FAILED');if(window.__mode==='pending')return new Promise(resolve=>window.__pending=()=>resolve(snapshot));return snapshot;}if(method==='inspection.check'){if(params.revision!==snapshot.revision||params.scope!=='mods')throw Error('Invalid revision/scope');return report;}if(method==='report.cancel')return null;throw Error('Unexpected method '+method);}};},{snapshot,report,narrow});
  await page.goto(`http://127.0.0.1:${server.address().port}/index.html`);
  await page.waitForFunction(()=>document.querySelector('#app')?.getAttribute('aria-busy')==='false'&&document.querySelectorAll('.mod-row').length>0);
  assert.equal(await page.locator('.mod-row').count(),100);assert.equal(await page.locator('.graph-node').count(),5);
  assert.match(await page.locator('.mod-details').innerText(),narrow?/无已声明匹配/:/No declared match/);
  assert.equal(await page.locator('.map-panel').isVisible(),!narrow);
  await page.locator('button').filter({hasText:narrow?'显示更多模组':'Show more Mods'}).click();assert.equal(await page.locator('.mod-row').count(),130);
  if(!narrow){await page.locator('input[type=search]').fill('Utility 000.jar');await page.locator('.mod-row').click();assert.equal(await page.locator('.graph-node').count(),40);await page.getByRole('button',{name:'Expand neighborhood',exact:true}).click();assert.equal(await page.locator('.graph-node').count(),80);await page.locator('input[type=search]').fill('');}
  await page.locator('.filters select').selectOption('requirements');assert.equal(await page.locator('.mod-row').count(),1);
  await page.locator('.mod-details').getByRole('button',{name:'World API',exact:true}).click();assert.equal(await page.locator('.filters select').inputValue(),'all');assert.equal(await page.locator('.mod-details h2').innerText(),'World API');
  await page.locator('.filters select').selectOption('all');await page.locator('input[type=search]').fill('WORLD-API');assert.equal(await page.locator('.mod-row').count(),2);
  await page.locator('input[type=search]').fill('World API.jar');assert.equal(await page.locator('.mod-row').count(),1);await page.locator('.mod-row').click();assert.match(await page.locator('.mod-details').innerText(),/Adventure Atlas/);
  await page.keyboard.press('Control+f');assert.equal(await page.locator('input[type=search]').evaluate(element=>element===document.activeElement),true);
  await page.locator('input[type=search]').fill('');await page.locator('.mod-row').first().focus();await page.keyboard.press('Enter');assert.equal(await page.locator('.mod-row[data-selected=true]').evaluate(element=>element===document.activeElement),true);
  if(!narrow){await page.locator('.graph-node').first().focus();await page.keyboard.press('Enter');assert.equal(await page.locator('.graph-node[data-selected=true]').evaluate(element=>element===document.activeElement),true);}
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);assert.deepEqual(errors,[]);
  const screenshot=`${narrow?'narrow-dark-zh':'wide-light-en'}.png`;await page.screenshot({path:path.join(output,screenshot),fullPage:true});
  await page.evaluate(()=>window.__mode='pending');await page.getByRole('button',{name:narrow?'刷新':'Refresh',exact:true}).click();await page.waitForFunction(()=>typeof window.__pending==='function');await page.getByRole('button',{name:narrow?'取消':'Cancel',exact:true}).click();await page.evaluate(()=>window.__pending());await page.waitForTimeout(30);assert.match(await page.locator('.status').innerText(),narrow?/已取消/:/Cancelled/);
  await page.evaluate(()=>window.__mode='error');await page.getByRole('button',{name:narrow?'刷新':'Refresh',exact:true}).click();await page.waitForFunction(()=>!document.querySelector('.error').hidden);assert.equal(await page.locator('#app').getAttribute('aria-busy'),'false');
  await page.evaluate(()=>window.__mode='normal');await page.getByRole('button',{name:narrow?'刷新':'Refresh',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.error').hidden&&document.querySelector('#app').getAttribute('aria-busy')==='false');
  result.checks.push({width:narrow?540:1440,locale:narrow?'zh-CN':'en',theme:narrow?'dark':'light',selectedNeighborhood:5,initialGraphBound:40,expandedGraphBound:narrow?null:80,inventory:130,keyboard:true,cancelDropsStaleSnapshot:true,errorRetry:true,screenshot,pageErrors:errors});await page.close();
 }
 result.passed=true;writeFileSync(path.join(output,'render-smoke.json'),JSON.stringify(result,null,2)+'\n');process.stdout.write(`Passed dependency workspace layouts and interactions: ${output}\n`);
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
