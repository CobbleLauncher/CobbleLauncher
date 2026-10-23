// SPDX-License-Identifier: MIT
// Replays captured native snapshots and actual Wasm reports through a synthetic
// SDK transport. This verifies rendered layouts; native proof remains separate.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFileSync,mkdirSync,writeFileSync,existsSync} from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const require=createRequire(new URL('../../apps/desktop/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const ids=['mod-conflicts','resource-pack-stack','world-dashboard','backup-auditor','screenshot-timeline','storage-lens','runtime-advisor','controls-auditor','datapack-auditor','serverbook-inspector'];
const input=path.resolve(process.argv[2]??'target/ui-evidence/inspection-native');
const output=path.resolve(process.argv[3]??'target/ui-evidence/inspection-render');
const requested=process.argv.slice(4);
if(requested.some(id=>!ids.includes(id))||new Set(requested).size!==requested.length)throw Error('Unknown or duplicate plugin ID');
const selected=requested.length?requested:ids;
const previous=existsSync(path.join(output,'render-all.json'))?JSON.parse(readFileSync(path.join(output,'render-all.json'),'utf8')):null;
if(requested.length&&!previous?.passed)throw Error('Subset rendering requires an existing complete successful render-all.json');
const captures=selected.map(id=>{const file=path.join(input,`${id}.json`);if(!existsSync(file))throw Error(`Native proof not ready: ${file}`);const proof=JSON.parse(readFileSync(file,'utf8'));const ui=proof.ui??proof.proof?.ui;const snapshot=ui?.snapshot?.Ok,report=ui?.report?.Ok;if(!snapshot?.revision||!Array.isArray(report?.findings))throw Error(`Native snapshot/report missing: ${file}`);return{id,file,snapshot,report};});
mkdirSync(output,{recursive:true});
const server=createServer((req,res)=>{try{let file;if(req.url==='/overview.html')file=path.join(output,'overview.html');else if(req.url?.startsWith('/captures/'))file=path.join(output,path.basename(req.url));else{const parts=decodeURIComponent(req.url??'').split('/').filter(Boolean),id=parts.shift();if(!ids.includes(id))throw Error('unknown package');const root=path.resolve(`target/${id}-package`);file=path.resolve(root,parts.join('/'));if(!file.startsWith(root+path.sep))throw Error('outside package');}res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':file.endsWith('.png')?'image/png':'text/html');res.end(readFileSync(file));}catch{res.writeHead(404);res.end();}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;const result={syntheticTransport:true,nativeDataReplay:true,limitations:['Actual captured native fixture snapshots and Wasm Report values are replayed into actual built package assets.','Chinese layout uses translated UI labels; captured Wasm report text retains its original native locale.'],checks:[]};
try{browser=await chromium.launch({headless:true,channel:'msedge'});
for(const capture of captures){for(const narrow of [false,true]){
 const page=await browser.newPage({viewport:narrow?{width:560,height:900}:{width:1280,height:900}});const errors=[];page.on('pageerror',error=>errors.push(String(error)));
 await page.addInitScript(({snapshot,report,narrow})=>{window.__renderCalls=[];window.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{window.__renderCalls.push({method,params});if(method==='context')return{locale:narrow?'zh-CN':'en',theme:narrow?'dark':'light',instanceId:snapshot.game.id,games:[snapshot.game],inspectionGames:[snapshot.game]};if(method==='inspection.snapshot'){if(params.gameId!==snapshot.game.id||params.scope!==snapshot.scope)throw Error('Snapshot scope mismatch');return snapshot;}if(method==='inspection.check'){if(params.revision!==snapshot.revision||params.scope!==snapshot.scope)throw Error('Analysis revision/scope mismatch');return report;}if(method==='report.cancel')return null;throw Error('Unexpected request '+method);}};},{snapshot:capture.snapshot,report:capture.report,narrow});
 await page.goto(`http://127.0.0.1:${server.address().port}/${capture.id}/ui/index.html`);
 await page.waitForFunction(()=>window.__renderCalls.some(call=>call.method==='inspection.check')&&document.querySelector('#cancel')?.hidden&&document.querySelector('#app')?.getAttribute('aria-busy')==='false');
 const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);assert.equal(overflow,false,`${capture.id} page overflow`);assert.deepEqual(errors,[],`${capture.id} page errors`);assert.equal(await page.locator('.error:visible').count(),0,`${capture.id} UI error`);
 const file=`${capture.id}-${narrow?'narrow-dark-zh':'wide-light-en'}.png`;await page.screenshot({path:path.join(output,file),fullPage:true});
 result.checks.push({id:capture.id,viewport:narrow?560:1280,locale:narrow?'zh-CN':'en',theme:narrow?'dark':'light',nativeProof:path.relative(process.cwd(),capture.file),inventoryRows:await page.locator('#inventory tbody tr').count(),findingCount:await page.locator('.finding').count(),reportTables:await page.locator('.report-table').count(),pageOverflow:overflow,pageErrors:errors,screenshot:file});await page.close();
}}
result.checks=[...(requested.length?previous.checks.filter(check=>!selected.includes(check.id)):[]),...result.checks].sort((left,right)=>ids.indexOf(left.id)-ids.indexOf(right.id)||right.viewport-left.viewport);
result.passed=true;writeFileSync(path.join(output,'render-all.json'),JSON.stringify(result,null,2)+'\n');
const panels=ids.map(id=>`<section><h2>${id}</h2><div class="pair"><a href="${id}-wide-light-en.png"><img src="${id}-wide-light-en.png" alt="${id} wide English light"></a><a href="${id}-narrow-dark-zh.png"><img src="${id}-narrow-dark-zh.png" alt="${id} narrow Chinese dark"></a></div></section>`).join('');
writeFileSync(path.join(output,'overview.html'),`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Inspection plugin layout review</title><style>body{font:14px system-ui;background:#eee;margin:24px}h1{font-size:20px}h2{font-size:16px}.pair{display:grid;grid-template-columns:2.28fr 1fr;gap:16px;align-items:start}section{margin:24px 0}img{width:100%;display:block;border:1px solid #ccc}p{max-width:80ch}</style><h1>Inspection plugin layout review</h1><p>Actual built plugin assets with synthetic SDK transport replaying captured native snapshots and real Wasm reports. Chinese UI labels retain the captured report's original language. Open images for full size.</p>${panels}</html>`);
console.log(`Rendered ${selected.length*2} layouts; retained ${result.checks.length} total: ${output}`);
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
