// SPDX-License-Identifier: MIT
import {createRequire} from 'node:module';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {mkdir,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const require=createRequire(resolve('apps/desktop/package.json'));
const {createServer}=await import(pathToFileURL(require.resolve('vite')).href);
const {chromium}=require('@playwright/test');
const server=await createServer({configFile:false,root:resolve('examples/geometry-builder/ui'),server:{host:'127.0.0.1',port:0},clearScreen:false});await server.listen();
const out=resolve('examples/geometry-builder/validation');await mkdir(out,{recursive:true});
const browser=await chromium.launch({channel:'msedge',headless:true});const receipt=[];
try{for(const [locale,theme,width] of [['en','light',1100],['zh-CN','dark',560]]){
 const page=await browser.newPage({viewport:{width,height:1000}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(({locale,theme})=>{globalThis.calls=[];globalThis.__COBBLE_PLUGIN_BRIDGE__={request:async(method,params)=>{calls.push({method,params});if(method==='context')return {locale,theme,instanceId:null,games:[]};if(method==='generatedBlueprint.save')return {blueprintId:'fixture-blueprint',name:params.name};if(method==='generatedBlueprint.open'){if(locale==='zh-CN')throw Error('NAVIGATION_CANCELLED');return null;}throw Error(method);}};},{locale,theme});
 await page.goto(server.resolvedUrls.local[0]);await page.locator('[data-action=generate]').click();await page.locator('canvas').waitFor({state:'visible'});
 assert.equal(await page.locator('canvas').getAttribute('role'),'img');
 await page.locator('[data-field=shape]').selectOption('sphere');await page.locator('[data-action=generate]').click();await page.locator('canvas').waitFor({state:'visible'});
 await page.locator('input[type=range]').fill('12');await page.locator('input[type=range]').dispatchEvent('input');
 await page.screenshot({path:resolve(out,`${locale}-${theme}.png`),fullPage:true});
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 await page.locator('[data-action=save]').click();await page.waitForFunction(()=>globalThis.calls.some(c=>c.method==='generatedBlueprint.open'));
 const saved=await page.evaluate(()=>globalThis.calls.find(c=>c.method==='generatedBlueprint.save').params);assert.equal(saved.width,25);assert(saved.occupancy.length>0);assert.deepEqual(errors,[]);
 assert.equal(await page.locator('[data-action=save]').isDisabled(),true);await page.locator('[data-action=open]').click();assert.equal(await page.evaluate(()=>globalThis.calls.filter(c=>c.method==='generatedBlueprint.save').length),1);
 await page.locator('[data-field=shape]').selectOption('dome');await page.locator('[data-field=width]').fill('128');await page.locator('[data-field=width]').dispatchEvent('change');await page.locator('[data-action=generate]').click();await page.getByRole('button',{name:locale==='zh-CN'?'取消':'Cancel',exact:true}).click();assert.equal(await page.locator('section').isVisible(),false);assert.equal(await page.locator('[data-action=save]').isDisabled(),true);
 receipt.push({locale,theme,width,shape:'sphere',saved:true,cancellation:true,horizontalOverflow:false});await page.close();
}await writeFile(resolve(out,'receipt.json'),JSON.stringify({kind:'synthetic browser fixture; no native library writes',checks:receipt},null,2));}finally{await browser.close();await server.close();}
