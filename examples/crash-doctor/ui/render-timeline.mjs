// SPDX-License-Identifier: MIT
// Replay actual Wasm report values through synthetic diagnostic transport.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const input=path.resolve(process.argv[2]??path.join(root,'target/ui-evidence/crash-timeline/report.json'));
const proof=JSON.parse(readFileSync(input,'utf8').replace(/^\uFEFF/,''));assert.equal(proof.plugin_id,'crash-doctor');assert.equal(proof.report.tables.length,4);
const log=readFileSync(path.join(root,'examples/crash-doctor/fixtures/synthetic-timeline.log'),'utf8');
const require=createRequire(new URL('../../../apps/desktop/package.json',import.meta.url)),{chromium}=require('@playwright/test');
const output=path.join(root,'target/ui-evidence/crash-timeline');mkdirSync(output,{recursive:true});
const dist=path.join(root,'examples/crash-doctor/ui/dist'),server=createServer((request,response)=>{try{const file=path.resolve(dist,decodeURIComponent((request.url??'/').split('?')[0]).slice(1));if(!file.startsWith(dist+path.sep))throw Error('outside bundle');response.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');response.end(readFileSync(file));}catch{response.writeHead(404);response.end();}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));let browser;
const result={syntheticTransport:true,actualWasmReport:input,nativeIpcTested:false,checks:[]};
try{browser=await chromium.launch({headless:true,channel:'msedge'});
 for(const narrow of [false,true]){
  const page=await browser.newPage({viewport:narrow?{width:520,height:980}:{width:1300,height:1050}}),errors=[];page.on('pageerror',error=>errors.push(String(error)));
  await page.addInitScript(({report,log,narrow})=>{window.__calls=[];window.__mode='normal';window.__pending=null;window.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{window.__calls.push({method,params});if(method==='context')return{locale:narrow?'zh-CN':'en',theme:narrow?'dark':'light',instanceId:'synthetic',games:[],recentGames:[{id:'synthetic',name:'Synthetic launch log',version:'1.21.1',lastLaunchedAt:1}]};if(method==='diagnostics.list')return[{id:'crash',name:'crash-reports/synthetic.txt',size:100,modifiedAt:2},{id:'latest',name:'logs/latest.log',size:log.length,modifiedAt:1}];if(method==='diagnostics.read')return{id:params.diagnosticId,name:params.diagnosticId==='latest'?'logs/latest.log':'synthetic.txt',text:log,revision:'a'.repeat(64)};if(method==='diagnostics.check'){if(window.__mode==='pending')return new Promise(resolve=>window.__pending=()=>resolve(report));if(window.__mode==='changed')throw Error('DIAGNOSTIC_CHANGED');return report;}if(method==='diagnostics.pick')return report;if(method==='report.cancel')return null;throw Error('Unexpected '+method);}};},{report:proof.report,log,narrow});
  await page.goto(`http://127.0.0.1:${server.address().port}/index.html`);await page.waitForFunction(()=>!document.querySelector('#check').disabled);
  assert.equal(await page.locator('#name').innerText(),'logs/latest.log');assert.equal(await page.evaluate(()=>window.__calls.find(call=>call.method==='diagnostics.read').params.diagnosticId),'latest');
  await page.locator('#timeline-tab').click();await page.waitForFunction(()=>document.querySelectorAll('.stage-event').length===8&&document.querySelector('#cancel').hidden);
  assert.equal(await page.locator('.gap-card').count(),2);assert.equal(await page.locator('.cluster-card').count(),1);assert.equal(await page.locator('#timeline-tab').getAttribute('aria-selected'),'true');
  assert.match(await page.locator('.timeline-stats').innerText(),/44 s/);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  await page.locator('.stage-event').nth(4).click();assert.equal(await page.locator('#source').evaluate(element=>element===document.activeElement),true);assert.match(await page.locator('#source').innerText(),/Reloading ResourceManager/);
  await page.locator('#diagnosis-tab').focus();await page.keyboard.press('ArrowRight');assert.equal(await page.locator('#timeline-tab').evaluate(element=>element===document.activeElement),true);
  const screenshot=narrow?'narrow-dark-zh.png':'wide-light-en.png';await page.screenshot({path:path.join(output,screenshot),fullPage:true});
  await page.evaluate(()=>window.__mode='pending');await page.locator('#check').click();await page.waitForFunction(()=>typeof window.__pending==='function');await page.locator('#cancel').click();await page.evaluate(()=>window.__pending());await page.waitForFunction(()=>document.querySelector('#cancel').hidden);assert.equal(await page.locator('.stage-event').count(),0);assert.match(await page.locator('#status').innerText(),narrow?/已取消/:/cancelled/);
  await page.evaluate(()=>window.__mode='changed');await page.locator('#check').click();await page.waitForFunction(()=>!document.querySelector('#error').hidden);assert.match(await page.locator('#error').innerText(),narrow?/文件已变化/:/file changed/);
  await page.evaluate(()=>window.__mode='normal');await page.locator('#manual').click();await page.waitForFunction(()=>document.querySelectorAll('.stage-event').length===8);assert.equal(await page.locator('#name').innerText(),narrow?'所选文件':'Selected file');assert.equal(await page.locator('.stage-event:disabled').count(),8);assert.equal(await page.locator('#source').innerText(),'');assert.deepEqual(errors,[]);
  result.checks.push({width:narrow?520:1300,locale:narrow?'zh-CN':'en',theme:narrow?'dark':'light',stageEvents:8,longGaps:2,warningClusters:1,latestLogPreferred:true,evidenceSourceJump:true,keyboardTabs:true,cancelDropsLateResult:true,changedRevisionError:true,manualSourceNotMisattributed:true,pageOverflow:false,pageErrors:errors,screenshot});await page.close();
 }
 result.passed=true;writeFileSync(path.join(output,'render-timeline.json'),JSON.stringify(result,null,2)+'\n');process.stdout.write(`Timeline layouts and interactions passed: ${output}\n`);
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
