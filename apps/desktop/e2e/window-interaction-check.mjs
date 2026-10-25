// SPDX-License-Identifier: MIT
// Browser geometry/input evidence only; this does not verify OS resize behavior.
import { createServer } from 'vite';
import { chromium } from '@playwright/test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
const server = await createServer({ server:{port:1423,strictPort:true},logLevel:'error' });
await server.listen();
let browser;
try {
 browser = await chromium.launch({channel:'msedge',headless:true});
 await mkdir('test-results',{recursive:true});
 const evidence = [];
 for(const [width,height,scale] of [[1100,760,1],[560,800,1.25]]) {
  const page = await browser.newPage({viewport:{width,height}});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(`http://127.0.0.1:1423/e2e/fixtures/window-interaction.html?scale=${scale}`);
  await page.locator('[data-resize-direction="North"]').waitFor();
  const actual = await page.evaluate(() => {
   const nodes = Object.fromEntries([...document.querySelectorAll('[data-resize-direction]')].map(element=>{
    const bounds=element.getBoundingClientRect();
    return [element.dataset.resizeDirection,{x:bounds.x,y:bounds.y,right:bounds.right,bottom:bounds.bottom,width:bounds.width,height:bounds.height,cursor:getComputedStyle(element).cursor}];
   }));
   const selectable=Object.fromEntries(['ordinary','editable','path','error','log','selectable','contenteditable','normal-button'].map(id=>[id,getComputedStyle(document.getElementById(id)).userSelect]));
   const button=document.getElementById('window-button').getBoundingClientRect();
   const drag=document.getElementById('drag-region').getBoundingClientRect();
   return {nodes,selectable,buttonHit:document.elementFromPoint(button.x+button.width/2,button.y+button.height/2)?.id,dragHit:document.elementFromPoint(drag.x+drag.width/2,drag.y+drag.height/2)?.id,overflow:document.documentElement.scrollWidth>innerWidth};
  });
  assert.equal(actual.nodes.North.y,0);assert.equal(actual.nodes.North.height,4);
  assert.equal(actual.nodes.East.right,width);assert.equal(actual.nodes.South.bottom,height);
  assert.equal(actual.nodes.West.x,0);assert.equal(actual.nodes.NorthWest.width,8);
  assert.equal(actual.nodes.SouthEast.right,width);assert.equal(actual.nodes.SouthEast.bottom,height);
  for(const [direction,cursor] of Object.entries({North:'n-resize',NorthEast:'ne-resize',East:'e-resize',SouthEast:'se-resize',South:'s-resize',SouthWest:'sw-resize',West:'w-resize',NorthWest:'nw-resize'}))assert.equal(actual.nodes[direction].cursor,cursor);
  assert.equal(actual.buttonHit,'window-button');assert.equal(actual.dragHit,'drag-region');
  assert.equal(actual.selectable.ordinary,'none');assert.equal(actual.selectable['normal-button'],'none');
  for(const id of ['editable','path','error','log','selectable','contenteditable'])assert.equal(actual.selectable[id],'text');
  assert.equal(actual.overflow,false);assert.deepEqual(errors,[]);
  for(const [id,expected] of [['ordinary',false],['path',true],['error',true],['log',true],['selectable',true]]) {
   await page.evaluate(()=>getSelection().removeAllRanges());
   const box=await page.locator(`#${id}`).boundingBox();
   await page.mouse.move(box.x+2,box.y+box.height/2);
   await page.mouse.down();await page.mouse.move(box.x+Math.min(box.width-2,250),box.y+box.height/2,{steps:8});await page.mouse.up();
   assert.equal(Boolean(await page.evaluate(()=>getSelection().toString())),expected,`${id} drag selection`);
  }
  await page.evaluate(()=>getSelection().removeAllRanges());
  await page.locator('#editable').fill('Changed world');
  assert.equal(await page.locator('#editable').inputValue(),'Changed world');
  await page.screenshot({path:`test-results/t078-window-interaction-${width}-${scale}.png`,fullPage:true});
  evidence.push({width,height,scale,...actual});
  await page.close();
 }
 await writeFile('test-results/t078-window-interaction.json',JSON.stringify(evidence,null,2));
 console.log('PASS: 8-edge geometry at100%/125%, cursor CSS, control hit-testing, selection exceptions, input edit, no overflow/page errors. Browser only.');
} finally { await browser?.close();await server.close(); }
