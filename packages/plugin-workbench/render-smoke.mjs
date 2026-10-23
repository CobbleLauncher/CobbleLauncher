// SPDX-License-Identifier: MIT
// Synthetic transport UI proof. Does not replace native/Wasm integration checks.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const require=createRequire(new URL('../../apps/desktop/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const id=process.argv[2]??'mod-conflicts',root=path.resolve(`examples/${id}/ui/dist`);
const output=path.resolve('apps/desktop/validation/plugin-workbench-20261023');mkdirSync(output,{recursive:true});
const server=createServer((req,res)=>{try{const name=req.url==='/'?'index.html':decodeURIComponent(req.url.slice(1));const file=path.resolve(root,name);if(!file.startsWith(root+path.sep))throw Error('outside build');res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');res.end(readFileSync(file));}catch{res.writeHead(404);res.end();}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;const proof={syntheticTransport:true,plugin:id,checks:[]};
try{browser=await chromium.launch({headless:true,channel:'msedge'});
for(const zh of [false,true]){
 const page=await browser.newPage({viewport:zh?{width:430,height:850}:{width:1240,height:820}});
 await page.addInitScript(({zh})=>{
  window.__workbenchCalls=[];let resolveCheck;let checks=0;
  window.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{
   window.__workbenchCalls.push({method,params});
   const game={id:'fixture-game',name:zh?'冒险世界':'Adventure world',version:'1.21.1'};
   if(method==='context')return{locale:zh?'zh-CN':'en',theme:zh?'dark':'light',instanceId:null,games:[],inspectionGames:[game]};
   if(method==='inspection.snapshot'){await new Promise(r=>setTimeout(r,120));return{schemaVersion:1,scope:'mods',game,data:{items:[{id:'fabric-api',name:'Fabric API',version:'0.102.1',loader:'Fabric',disabled:false},{id:'sodium',name:'Sodium',version:'0.6.0',loader:'Fabric',disabled:false},{id:'iris',name:'Iris Shaders',version:'1.8.0',loader:'Fabric',disabled:true}],truncated:false},revision:'fixture-revision'};}
   if(method==='inspection.check'){checks++;if(checks>1)return new Promise(resolve=>resolveCheck=resolve);return{schema_version:1,findings:[{code:'fixture-disabled',severity:'warning',title:zh?'已禁用的模组':'Disabled mod',message:zh?'Iris Shaders 当前未启用。':'Iris Shaders is currently disabled.',evidence:[],suggestions:[zh?'如需光影效果，请在模组管理中启用。':'Enable it in mod management when you want shaders.']}],tables:[]};}
   if(method==='report.cancel'){resolveCheck?.(null);return null;}
   throw Error('Unexpected method '+method);
  }};
 },{zh});
 await page.goto(`http://127.0.0.1:${server.address().port}/`);
 await page.locator('.finding').waitFor();
 assert.equal(await page.locator('tbody tr').count(),3);
 await page.locator('input[type=search]').fill('sodium');assert.equal(await page.locator('tbody tr').count(),1);
 await page.locator('input[type=search]').fill('');await page.getByRole('button',{name:'Iris Shaders',exact:true}).click();assert.equal(await page.locator('.detail[open]').count(),1);
 await page.screenshot({path:path.join(output,zh?'workbench-narrow-dark-zh.png':'workbench-wide-light.png'),fullPage:true});
 await page.getByRole('button',{name:zh?'刷新':'Refresh',exact:true}).click();assert.equal(await page.locator('tbody tr').count(),3);
 await page.getByRole('button',{name:zh?'取消分析':'Cancel analysis',exact:true}).waitFor();await page.getByRole('button',{name:zh?'取消分析':'Cancel analysis',exact:true}).click();
 await page.getByText(zh?'已取消分析，刷新后可重试。':'Analysis cancelled. Refresh to try again.',{exact:true}).waitFor();
 const calls=await page.evaluate(()=>window.__workbenchCalls);const check=calls.find(call=>call.method==='inspection.check');assert.equal(check.params.revision,'fixture-revision');assert(calls.some(call=>call.method==='report.cancel'));
 proof.checks.push({locale:zh?'zh-CN':'en',autoSnapshot:true,analysisWithDisplayedRevision:true,search:true,rowDetails:true,refreshPreservesInventory:true,cancel:true});await page.close();
}
proof.passed=true;writeFileSync(path.join(output,'render-smoke.json'),JSON.stringify(proof,null,2)+'\n');
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
