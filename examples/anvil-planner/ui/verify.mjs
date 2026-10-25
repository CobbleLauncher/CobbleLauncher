// SPDX-License-Identifier: MIT
// Production assets in the native-shaped opaque sandbox/CSP; synthetic SDK bridge only.
import {createServer} from 'node:http';
import {readFile,mkdir,writeFile,stat} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {createRequire} from 'node:module';
import assert from 'node:assert/strict';
const require=createRequire(resolve('apps/desktop/package.json'));
const {chromium}=require('@playwright/test');
const root=resolve('examples/anvil-planner/ui/dist'),out=resolve('examples/anvil-planner/validation');await mkdir(out,{recursive:true});
let base='';const failures=[];
const server=createServer(async(req,res)=>{
 try{
  res.setHeader('Access-Control-Allow-Origin','null');res.setHeader('Cache-Control','no-store');res.setHeader('X-Content-Type-Options','nosniff');
  if(req.url==='/'){
   res.setHeader('Content-Type','text/html');res.end('<!doctype html><html><head><style>html,body,iframe{width:100%;height:100%;margin:0;border:0}</style></head><body><iframe sandbox="allow-scripts" title="Plugin" src="/ui/index.html"></iframe></body></html>');return;
  }
  const path=resolve(root,decodeURIComponent(req.url.split('?')[0].replace(/^\/ui\//,'')));
  if(!path.startsWith(root+sep))throw Error('OUTSIDE_FIXTURE_ROOT');
  res.setHeader('Content-Security-Policy',`default-src 'none'; script-src ${base}; style-src ${base} 'unsafe-inline'; img-src ${base} data:; font-src ${base} data:; connect-src 'none'; worker-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'`);
  res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.json':'application/json'})[extname(path)]??'application/octet-stream');res.end(await readFile(path));
 }catch(error){failures.push(String(error));res.statusCode=404;res.end();}
});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));base=`http://127.0.0.1:${server.address().port}`;
const browser=await chromium.launch({channel:'msedge',headless:true}),receipts=[];
async function fixture(locale='en',theme='light',width=1280,version='1.21.1'){
 const page=await browser.newPage({viewport:{width,height:1100}}),consoleErrors=[];
 page.on('pageerror',error=>consoleErrors.push(String(error)));page.on('console',message=>{if(message.type()==='error')consoleErrors.push(message.text());});
 await page.route('**/*',route=>route.request().url().startsWith(base)?route.continue():route.abort());
 await page.addInitScript(({locale,theme,version})=>{
  globalThis.calls=[];globalThis.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{
   calls.push({method,params:structuredClone(params??{})});
   if(method==='context')return{locale,theme,instanceId:'fixture',games:[{id:'fixture',name:'Fixture',version}]};
   if(method==='clipboard.writeText'){if(globalThis.denyClipboard)throw Error('PERMISSION_DENIED');return null;}
   throw Error(`UNEXPECTED:${method}`);
  }};
 },{locale,theme,version});
 await page.goto(base);const frame=await page.frame({url:base+'/ui/index.html'});assert.ok(frame);await frame.locator('.anvil-planner[aria-busy=false]').waitFor();
 return {page,frame,consoleErrors};
}
async function screenshot(page,frame,name){
 assert.equal(await frame.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
 const height=await frame.evaluate(()=>document.documentElement.scrollHeight);await page.setViewportSize({width:page.viewportSize().width,height:Math.max(900,Math.min(6000,height))});
 await frame.evaluate(()=>scrollTo(0,0));await page.evaluate(()=>scrollTo(0,0));
 await frame.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 // Opaque cross-origin frames have an independent compositor; settle its resize before capture.
 await page.waitForTimeout(100);
 await page.screenshot({path:resolve(out,name),fullPage:true});
 assert.ok((await stat(resolve(out,name))).size>15_000,'Rendered evidence must include the plugin surface');
}
async function compute(frame){await frame.locator('[data-action=calculate]').click();await frame.locator('.anvil-planner[aria-busy=false]').waitFor();}
try{
 for(const sample of [{locale:'en',theme:'light',width:1280},{locale:'zh-CN',theme:'dark',width:560}]){
  const {page,frame,consoleErrors}=await fixture(sample.locale,sample.theme,sample.width);
  assert.equal(await frame.locator('[data-field=version]').inputValue(),'1.21.1');await compute(frame);
  assert.equal(await frame.locator('.metric strong').first().textContent(),'14');assert.equal(await frame.locator('.steps li').count(),3);
  await screenshot(page,frame,`anvil-${sample.locale}-${sample.width}.png`);await frame.locator('[data-action=copy]').click();
  assert.match(await frame.evaluate(()=>calls.find(call=>call.method==='clipboard.writeText').params.text),/14/);
  await frame.evaluate(()=>globalThis.denyClipboard=true);await frame.locator('[data-action=copy]').click();assert.match(await frame.locator('.status').textContent(),sample.locale==='en'?/Could not copy/:/无法复制/);
  await frame.locator('.existing-details summary').click();await frame.locator('[data-action=add-existing]').click();await frame.locator('.existing-details [data-field=level]').selectOption('4');
  await frame.locator('.input-panel>.entries [data-field=level]').first().selectOption('4');await frame.locator('[data-field=item-work]').selectOption('2');await compute(frame);
  assert.match(await frame.locator('.final-enchants').textContent(),sample.locale==='en'?/Sharpness 5/:/锋利 5/);
  await frame.locator('.input-panel>.entries [data-field=enchantment]').nth(1).selectOption('smite');await compute(frame);
  await frame.locator('.error').waitFor({state:'visible'});assert.equal(await frame.locator('.steps li').count(),0);await screenshot(page,frame,`anvil-conflict-${sample.locale}-${sample.width}.png`);
  await frame.locator('[data-action=preset]').click();
  await frame.locator('.input-panel>.entries [data-action=remove]').nth(2).click();await frame.locator('.input-panel>.entries [data-action=remove]').nth(1).click();
  await frame.locator('.input-panel>.entries [data-field=enchantment]').first().selectOption('looting');await frame.locator('.input-panel>.entries [data-field=book-work]').selectOption('2');await frame.locator('[data-field=item-work]').selectOption('5');await compute(frame);
  assert.equal(await frame.locator('.metric strong').first().textContent(),'40');assert.equal(await frame.locator('.steps li[data-expensive=true]').count(),1);await screenshot(page,frame,`anvil-expensive-${sample.locale}-${sample.width}.png`);
  const calls=await frame.evaluate(()=>globalThis.calls);assert.ok(calls.every(call=>['context','clipboard.writeText'].includes(call.method)));assert.deepEqual(consoleErrors,[]);
  receipts.push({...sample,productionOpaqueSandbox:true,workersBlocked:true,offline:true,result14:true,copy:true,clipboardDenied:true,existingUpgrade:true,priorWork:true,conflict:true,fortyLevelFallback:true,noHorizontalOverflow:true});await page.close();
 }
 const {page,frame,consoleErrors}=await fixture('en','light',560,'1.21.4');assert.match(await frame.locator('.notice').first().textContent(),/1.21.4/);
 await frame.locator('[data-field=item]').selectOption('boots');
 while(await frame.locator('.input-panel>.entries .entry').count()<8)await frame.locator('[data-action=add-book]').click();
 const ids=['protection','feather_falling','depth_strider','soul_speed','thorns','unbreaking','mending','vanishing_curse'];
 for(let i=0;i<8;i++)await frame.locator('.input-panel>.entries [data-field=enchantment]').nth(i).selectOption(ids[i]);
 await frame.evaluate(()=>{document.querySelector('[data-action=calculate]').click();document.querySelector('[data-action=cancel]').click();});
 assert.match(await frame.locator('.status').textContent(),/cancelled/);assert.equal(await frame.locator('.steps li').count(),0);
 await compute(frame);assert.equal(await frame.locator('.steps li').count(),8);assert.equal(await frame.locator('[data-action=add-book]').isDisabled(),true);
 await frame.locator('[data-field=item-work]').selectOption('1');assert.equal(await frame.locator('.steps li').count(),0);assert.deepEqual(consoleErrors,[]);
 receipts.push({unsupportedContextDisclosed:true,eightBooks:true,cancellation:true,reuseAfterCancel:true,editInvalidatesResult:true});await page.close();
 assert.deepEqual(failures,[]);await writeFile(resolve(out,'summary.json'),JSON.stringify({evidence:'Built production frontend; native-shaped sandbox allow-scripts (opaque origin), CSP connect-src/worker-src none. Synthetic SDK bridge; no native/game equivalence claim.',receipts},null,2));
 console.log(JSON.stringify({pass:receipts.length,evidence:resolve(out,'summary.json')}));
}finally{await browser.close();await new Promise(resolve=>server.close(resolve));}
