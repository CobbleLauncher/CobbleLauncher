// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { gzipSync } from 'node:zlib';
import { english, demoCopy, labels } from './dist/copy.js';
import { circleOutline } from './dist/demos.js';
const require=createRequire(new URL('../desktop/package.json',import.meta.url));
const {JSDOM}=require('jsdom');
const html=await readFile(new URL('./dist/index.html',import.meta.url),'utf8');
const document=new JSDOM(html).window.document;

test('all visible bilingual strings and accessible labels have English equivalents',()=>{
  for(const el of document.querySelectorAll('[data-i18n]'))assert.ok(english[el.dataset.i18n],el.dataset.i18n);
  for(const el of document.querySelectorAll('[data-label]'))for(const lang of ['zh','en'])assert.ok(labels[lang][el.dataset.label]);
  assert.deepEqual(Object.keys(demoCopy.en).sort(),Object.keys(demoCopy.zh).sort());
});
test('navigation anchors, tab associations and SVG references resolve',async()=>{
  const sprite=await readFile(new URL('./dist/icons.svg',import.meta.url),'utf8');
  const ids=[...document.querySelectorAll('[id]')].map(el=>el.id);assert.equal(new Set(ids).size,ids.length);
  for(const el of document.querySelectorAll('[aria-controls]'))assert.ok(document.getElementById(el.getAttribute('aria-controls')));
  for(const el of document.querySelectorAll('a[href^="#"]')){const id=el.hash.slice(1);if(id)assert.ok(document.getElementById(id),id);}
  for(const use of document.querySelectorAll('use'))assert.ok(sprite.includes(`id="${use.getAttribute('href').split('#')[1]}"`));
  for(const link of document.querySelectorAll('a[target="_blank"]'))assert.match(link.rel,/noopener/);
});
test('circle preview stays symmetric, unique and within the chosen radius',()=>{
  for(let radius=4;radius<=16;radius++){
    const cells=circleOutline(radius),set=new Set(cells.map(p=>p.join(',')));
    assert.equal(cells.length,set.size);assert.ok(cells.length>0);
    for(const [x,y] of cells){assert.ok(set.has(`${-x},${y}`));assert.ok(set.has(`${x},${-y}`));assert.ok(set.has(`${y},${x}`));assert.ok(Math.abs(Math.hypot(x,y)-radius)<.5);}
  }
});
test('static site is self-contained, small and contains no release placeholder downloads',async()=>{
  assert.equal(document.querySelectorAll('script[src^="http"],link[href^="http"]').length,0);
  assert.ok(!html.includes('.zip'));assert.ok(html.includes('尚未发布公开安装包'));
  let raw=0,gzip=0;
  for(const name of await readdir(new URL('./dist/',import.meta.url))){const data=await readFile(new URL(`./dist/${name}`,import.meta.url));raw+=data.length;gzip+=gzipSync(data).length;}
  assert.ok(raw<240_000,`raw bytes ${raw}`);assert.ok(gzip<85_000,`gzip bytes ${gzip}`);
  console.log(`Static assets: ${raw} bytes; individually gzipped ${gzip} bytes.`);
});

test('all static pages resolve local links, anchors and accessible diagrams',async()=>{
  const files=await readdir(new URL('./dist/',import.meta.url));
  const sprite=await readFile(new URL('./dist/icons.svg',import.meta.url),'utf8');
  for(const file of files.filter(name=>name.endsWith('.html'))){
    const source=await readFile(new URL(`./dist/${file}`,import.meta.url),'utf8');
    const doc=new JSDOM(source).window.document;
    assert.equal(doc.querySelectorAll('h1').length,1,file);
    assert.ok(doc.querySelector('html[lang]'),file);
    const ids=[...doc.querySelectorAll('[id]')].map(el=>el.id);
    assert.equal(new Set(ids).size,ids.length,file);
    for(const el of doc.querySelectorAll('[href],[src]')){
      const link=el.getAttribute('href')??el.getAttribute('src');
      if(link.startsWith('#')){if(link.length>1)assert.ok(doc.getElementById(link.slice(1)),`${file}: ${link}`);}
      if(link.startsWith('./')&&link!=='./')assert.ok(files.includes(link.slice(2).split('#')[0]),`${file}: ${link}`);
    }
    for(const el of doc.querySelectorAll('use'))assert.ok(sprite.includes(`id="${el.getAttribute('href').split('#')[1]}"`),file);
    for(const svg of doc.querySelectorAll('svg[role="img"]'))assert.ok(doc.getElementById(svg.getAttribute('aria-labelledby')),file);
    assert.equal(doc.querySelectorAll('script[src^="http"]').length,0,file);
  }
  for(const lang of ['','.en']){
    const mesh=await readFile(new URL(`./dist/mesh${lang}.html`,import.meta.url),'utf8');
    assert.equal(new JSDOM(mesh).window.document.querySelectorAll('figure svg').length,5);
    assert.ok(mesh.includes('/cobblemesh/game-stream/0.1.0'));
  }
});
