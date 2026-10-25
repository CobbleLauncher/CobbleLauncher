// SPDX-License-Identifier: MIT
// Actual source renderer with synthetic report/document bridge. Native Wasm proof is separate.
import {createRequire} from 'node:module';import{resolve}from'node:path';import{pathToFileURL}from'node:url';import{mkdir,writeFile,readFile}from'node:fs/promises';import assert from'node:assert/strict';
const require=createRequire(resolve('apps/desktop/package.json'));const{createServer}=await import(pathToFileURL(require.resolve('vite')).href);const{chromium}=require('@playwright/test');
const fixtureData=JSON.parse(await readFile('examples/lang-doctor/fixtures/comparison.json','utf8'));
const server=await createServer({configFile:false,root:resolve('examples/lang-doctor/ui'),server:{host:'127.0.0.1',port:0,cors:{origin:'*'}},clearScreen:false});await server.listen();
const out=resolve('examples/lang-doctor/validation');await mkdir(out,{recursive:true});const browser=await chromium.launch({channel:'msedge',headless:true});const receipts=[];
async function fixture(locale='en',theme='light',width=1280,scenario='normal'){
 const page=await browser.newPage({viewport:{width,height:940}});await page.route('https://**/*',route=>route.abort());
 await page.addInitScript(({locale,theme,scenario,data})=>{
  globalThis.calls=[];
  const zh=locale==='zh-CN';
  const report={schema_version:1,findings:[{code:'language_summary',severity:'warning',title:zh?'发现翻译问题':'Translation issues found',message:'',evidence:[],suggestions:[]}],tables:[{columns:['source','namespace','baseline','target','pack_format','reference_keys','target_keys','issue_count'],rows:[[data.sourceName,'example','en_us','zh_cn','34','3','3','5']]},{columns:['kind','key','line','reference','translation','message','file'],rows:[['missing_key','missing','3','Only in the reference','',zh?'基准中的键缺少翻译。':'This reference key has no translation.','file-0'],['placeholder_mismatch','greeting','2','Hello %s','你好',zh?'翻译缺少占位符 %s。':'The translation is missing placeholder %s.','file-1'],['placeholder_mismatch','count','3','%1$s / %2$d','%1$s / %2$s',zh?'占位符类型不一致。':'Placeholder types do not match.','file-1'],['extra_key','duplicate','4','','第一项',zh?'基准中不存在此键。':'This key is not in the reference.','file-1'],['duplicate_key','duplicate','5','','第二项',zh?'此键重复出现。':'This key occurs more than once.','file-1']]}]};
  globalThis.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{
   calls.push({method,params:structuredClone(params??{})});
   if(method==='context')return{locale,theme,instanceId:null,games:[]};
   if(method==='langPackage.pick'){
    if(scenario==='error')throw Error('LANG_PACKAGE_INVALID');
    return{selectionId:'a'.repeat(64),sourceName:data.sourceName,kind:scenario==='single'?'language_files':'resource_pack',packFormat:scenario==='single'?null:34,files:(scenario==='single'?[data.files[1]]:data.files).map(file=>({id:file.id,namespace:scenario==='single'?null:'example',locale:file.locale,name:file.name}))};
   }
   if(method==='langPackage.check'){
    if(scenario==='cancel')return await new Promise(resolve=>globalThis.resolveLate=()=>resolve(report));
    return scenario==='clean'?{...report,tables:[{...report.tables[0],rows:report.tables[0].rows.map(row=>[...row.slice(0,-1),'0'])},{...report.tables[1],rows:[]}]}:report;
   }
   if(method==='langPackage.document'){const file=data.files.find(file=>file.id===params.fileId);return{name:file.name,text:file.text};}
   if(method==='langPackage.cancel')return null;
   if(method==='exports.text')return{saved:false};
   throw Error(`UNEXPECTED:${method}`);
  }};
 },{locale,theme,scenario,data:fixtureData});await page.goto(server.resolvedUrls.local[0]);await page.locator('[data-action=pick]').waitFor();return page;
}
try{
 for(const sample of[{locale:'en',theme:'light',width:1280},{locale:'zh-CN',theme:'dark',width:560}]){
  const page=await fixture(sample.locale,sample.theme,sample.width);await page.locator('[data-action=pick]').click();
  await page.locator('[data-field=baseline]').selectOption('en_us');await page.locator('[data-action=check]').click();await page.locator('tbody tr').nth(4).waitFor();
  await page.getByRole('button',{name:'missing',exact:true}).click();await page.locator('.source-line[data-active=true]').waitFor();
  assert.equal(await page.evaluate(()=>calls.filter(c=>c.method==='langPackage.document').at(-1).params.fileId),'file-0');
  assert.equal(await page.locator('.source-line[data-active=true] .line-number').textContent(),'3');
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
  await page.screenshot({path:resolve(out,`language-doctor-${sample.locale}-${sample.width}.png`),fullPage:true});
  await page.locator('[data-field=filter]').selectOption('placeholder_mismatch');assert.equal(await page.locator('tbody tr').count(),2);
  await page.locator('[data-action=export]').click();assert.equal(await page.evaluate(()=>calls.filter(c=>c.method==='exports.text').at(-1).params.text.includes('duplicate_key')),true);
  assert.equal(await page.locator('.status').textContent(),'');
  await page.locator('[data-field=target]').selectOption('en_us');assert.equal(await page.locator('[data-action=check]').isDisabled(),true);
  receipts.push({...sample,sourceBaselineFileCorrect:true,exactLine:true,kindFilter:true,fullReportExport:true,cancelExportNoSuccess:true,sameLocaleBlocked:true});await page.close();
 }
 const cancel=await fixture('en','light',1000,'cancel');await cancel.locator('[data-action=pick]').click();await cancel.locator('[data-action=check]').click();await cancel.locator('[data-action=cancel]').click();await cancel.evaluate(()=>resolveLate());await cancel.waitForFunction(()=>document.querySelector('.language-doctor').getAttribute('aria-busy')==='false');assert.equal(await cancel.locator('tbody tr').count(),0);assert.equal(await cancel.locator('.source').textContent(),'');receipts.push({cancelClearsSelection:true,lateReportIgnored:true});await cancel.close();
 const single=await fixture('en','light',1000,'single');await single.locator('[data-action=pick]').click();assert.equal(await single.locator('[data-field=baseline]').inputValue(),'');await single.locator('[data-action=check]').click();await single.locator('tbody tr').first().waitFor();assert.equal(await single.evaluate(()=>calls.find(c=>c.method==='langPackage.check').params.baselineLocale),null);assert.equal(await single.evaluate(()=>calls.find(c=>c.method==='langPackage.check').params.namespace),null);receipts.push({singleFileSyntaxOnly:true,namespaceNotInvented:true});await single.close();
 for(const scenario of['empty','error','clean']){const page=await fixture('zh-CN','dark',560,scenario);if(scenario!=='empty'){await page.locator('[data-action=pick]').click();if(scenario==='clean'){await page.locator('[data-action=check]').click();await page.locator('.empty').filter({hasText:'未发现问题'}).waitFor();}else await page.locator('.error').waitFor({state:'visible'});}await page.screenshot({path:resolve(out,`language-doctor-${scenario}-zh-CN-560.png`),fullPage:true});await page.close();}
 await writeFile(resolve(out,'summary.json'),JSON.stringify({evidence:'actual source renderer with synthetic host/report. No native Wasm assertion.',receipts},null,2));console.log(JSON.stringify({pass:receipts.length,evidence:resolve(out,'summary.json')}));
}finally{await browser.close();await server.close();}
