// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {desktopReleases,detectDesktopOS,releasesPage,setupDownloads} from './dist/downloads.js';
const {JSDOM}=createRequire(new URL('../desktop/package.json',import.meta.url))('jsdom');
const html=await readFile(new URL('./dist/index.html',import.meta.url),'utf8');
function fixture(nav){
  const dom=new JSDOM(html),doc=dom.window.document,downloads=setupDownloads(doc,nav);downloads.setLanguage('zh');
  const key=(element,value)=>element.dispatchEvent(new dom.window.KeyboardEvent('keydown',{key:value,bubbles:true,cancelable:true}));
  return {dom,doc,downloads,key};
}

test('desktop detection handles reduced UA hints, mobile and iPad desktop mode conservatively',()=>{
  for(const [nav,expected] of [
    [{userAgentData:{platform:'Windows',mobile:false}},'windows'],[{platform:'Win32'},'windows'],
    [{platform:'MacIntel'},'macos'],[{userAgent:'Mozilla/5.0 (X11; Linux x86_64)'},'linux'],
    [{platform:'Linux',userAgent:'Android'},'unknown'],[{platform:'MacIntel',maxTouchPoints:5},'unknown'],
    [{userAgent:'iPhone'},'unknown'],[{userAgentData:{platform:'Windows',mobile:true}},'unknown'],[{},'unknown'],
  ])assert.equal(detectDesktopOS(nav),expected);
});
test('published asset list contains only the real Windows x64 Beta ZIP',()=>{
  assert.equal(desktopReleases.filter(item=>item.href).length,1);
  assert.equal(desktopReleases[0].href,'https://github.com/CobbleLauncher/CobbleLauncher/releases/download/v0.1.0-beta.1/CobbleLauncher-windows-x64.zip');
  assert.deepEqual(desktopReleases.slice(1).map(item=>item.href),[null,null]);
});
test('all Windows entries use the same asset and bilingual x64 label',()=>{
  const {dom,doc,downloads}=fixture({platform:'Win32'});
  assert.equal(doc.querySelectorAll('[data-get]').length,3);
  for(const link of doc.querySelectorAll('[data-get]')){assert.equal(link.tagName,'A');assert.equal(link.href,desktopReleases[0].href);assert.equal(link.textContent,'Windows x64 Beta');}
  downloads.setLanguage('en');assert.match(doc.querySelector('[data-get]').getAttribute('aria-label'),/^Download Windows x64/);assert.equal(doc.querySelector('.download-toggle').getAttribute('aria-label'),'Other systems');dom.window.close();
});
test('unreleased macOS and Linux link to releases without invented architectures or Windows relabeling',()=>{
  for(const platform of ['MacIntel','Linux x86_64']){
    const {dom,doc,downloads}=fixture({platform});downloads.setLanguage('en');
    for(const link of doc.querySelectorAll('[data-get]')){assert.equal(link.href,releasesPage);assert.match(link.textContent,/Not released yet/);assert.doesNotMatch(link.textContent,/Windows|arm|x64/);}
    assert.match(doc.querySelector('.download-menu').textContent,/macOSNot released yet/);dom.window.close();
  }
});
test('unknown/mobile main action opens the same menu; keyboard traversal and Escape restore focus',()=>{
  const {dom,doc,key}=fixture({userAgent:'Android Mobile'}),main=doc.querySelector('.arrival [data-get]'),toggle=doc.querySelector('.arrival .download-toggle'),panel=doc.querySelector('.download-menu');
  assert.equal(main.tagName,'BUTTON');assert.equal(main.textContent,'选择桌面系统');main.click();assert.equal(panel.hidden,false);assert.equal(doc.activeElement,panel.children[0]);
  key(doc.activeElement,'End');assert.equal(doc.activeElement,panel.children[2]);key(doc.activeElement,'ArrowDown');assert.equal(doc.activeElement,panel.children[0]);key(doc.activeElement,'Escape');assert.equal(panel.hidden,true);assert.equal(doc.activeElement,main);
  key(toggle,'ArrowUp');assert.equal(doc.activeElement,panel.children[2]);key(doc.activeElement,'Home');assert.equal(doc.activeElement,panel.children[0]);dom.window.close();
});
test('outside click, focus exit, selection and language change close the shared menu',()=>{
  const {dom,doc,downloads}=fixture({platform:'Win32'}),toggle=doc.querySelector('.arrival .download-toggle'),panel=doc.querySelector('.download-menu');
  toggle.click();doc.body.dispatchEvent(new dom.window.Event('pointerdown',{bubbles:true}));assert.equal(panel.hidden,true);
  toggle.click();doc.querySelector('#language').focus();assert.equal(panel.hidden,true);
  toggle.click();panel.children[0].addEventListener('click',e=>e.preventDefault());panel.children[0].click();assert.equal(panel.hidden,true);
  toggle.click();downloads.setLanguage('en');assert.equal(panel.hidden,true);assert.equal(panel.getAttribute('aria-label'),'Other systems');assert.match(panel.children[1].textContent,/Not released yet · View releases/);dom.window.close();
});
