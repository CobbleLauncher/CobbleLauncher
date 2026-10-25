// SPDX-License-Identifier: MIT
import { createServer } from 'vite';
import { chromium } from '@playwright/test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
const server=await createServer({server:{port:1424,strictPort:true},logLevel:'error'});
await server.listen();let browser;
try{
 browser=await chromium.launch({channel:'msedge',headless:true});
 await mkdir('test-results',{recursive:true});
 const evidence=[];
 for(const width of [1280,560])for(const reduced of [false,true]){
  const page=await browser.newPage({viewport:{width,height:800},reducedMotion:reduced?'reduce':'no-preference'});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(`http://127.0.0.1:1424/e2e/fixtures/shared-motion.html?theme=${width===560?'dark':'light'}`);
  await page.locator('.settings-disclosure-trigger').waitFor();
  const samples=await page.evaluate(async()=>{
   const trigger=document.querySelector('.settings-disclosure-trigger');
   trigger.click();const frames=[],start=performance.now();
   while(performance.now()-start<250){await new Promise(requestAnimationFrame);const node=document.querySelector('.settings-disclosure-motion');frames.push({time:performance.now()-start,height:node.getBoundingClientRect().height,opacity:getComputedStyle(node).opacity,duration:getComputedStyle(node).transitionDuration});}
   return frames;
  });
  const finalHeight=samples.at(-1).height;
  assert.ok(finalHeight>150);
  if(reduced)assert.ok(samples.every(frame=>frame.duration==='0s'));
  else assert.ok(samples.some(frame=>frame.height>1&&frame.height<finalHeight-1),'intermediate disclosure height');
  await page.locator('#draft-limit').fill('18');
  await page.evaluate(()=>{window.fixtureInput=document.querySelector('#draft-limit');document.querySelector('#draft-limit').focus();document.querySelector('.settings-disclosure-trigger').click();});
  assert.equal(await page.locator('.settings-disclosure-region').evaluate(node=>node.inert),true);
  assert.equal(await page.evaluate(()=>document.activeElement.className),'settings-disclosure-trigger');
  await page.locator('.settings-disclosure-motion').waitFor({state:'hidden'});
  await page.locator('.settings-disclosure-trigger').click();
  await page.locator('#draft-limit').waitFor({state:'visible'});
  assert.equal(await page.locator('#draft-limit').inputValue(),'18');
  assert.equal(await page.evaluate(()=>window.fixtureInput===document.querySelector('#draft-limit')),true);
  assert.equal(await page.locator('.settings-disclosure-region').evaluate(node=>node.inert),false);
  await page.waitForFunction(()=>{const node=document.querySelector('.settings-disclosure-motion');return node.getBoundingClientRect().height>150&&getComputedStyle(node).opacity==='1';});
  await page.locator('#draft-limit').focus();
  await page.screenshot({path:`test-results/t081-shared-disclosure-${width}-${reduced?'reduced':'motion'}.png`,animations:'disabled'});
  await page.locator('#reset-opener').click();
  const dialog=page.locator('.confirmation-dialog');await dialog.waitFor();
  const dialogAnimation=await dialog.evaluate(node=>({name:getComputedStyle(node).animationName,duration:getComputedStyle(node).animationDuration}));
  if(reduced)assert.equal(dialogAnimation.name,'none');else assert.ok(dialogAnimation.name.includes('confirmation-dialog-enter'));
  assert.equal(await page.evaluate(()=>document.activeElement.classList.contains('confirmation-cancel')),true);
  await page.screenshot({path:`test-results/t081-shared-dialog-${width}-${reduced?'reduced':'motion'}.png`,animations:'disabled'});
  await page.keyboard.press('Escape');
  if(!reduced){assert.equal(await dialog.count(),1);assert.equal(await dialog.getAttribute('data-state'),'closed');const exit=await dialog.evaluate(node=>getComputedStyle(node).animationName);assert.ok(exit.includes('confirmation-dialog-exit'));}
  await dialog.waitFor({state:'detached'});
  assert.equal(await page.evaluate(()=>document.activeElement.id),'reset-opener');
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  assert.deepEqual(errors,[]);
  evidence.push({width,reduced,samples,dialogAnimation,retainedDraft:true,retainedInputNode:true,restoredFocus:true,noOverflow:true});
  await page.close();
 }
 await writeFile('test-results/t081-shared-motion.json',JSON.stringify(evidence,null,2));
 console.log('PASS: wide/narrow disclosure height motion, retained draft/node/focus, dialog enter/exit and focus restore, reduced motion, no overflow/errors.');
}finally{await browser?.close();await server.close();}
