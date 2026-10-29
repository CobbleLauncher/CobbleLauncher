// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {desktopReleases,detectDesktopArch,detectDesktopOS,releasesApi,releasesPage,releaseCacheKey,releaseCacheTtl,setupDownloads,selectReleaseAssets} from './dist/downloads.js';
const {JSDOM}=createRequire(new URL('../desktop/package.json',import.meta.url))('jsdom');
const html=await readFile(new URL('./dist/index.html',import.meta.url),'utf8');
const copy=await readFile(new URL('./dist/copy.js',import.meta.url),'utf8');
const release=(tag,at,assets,prerelease=true)=>({tag_name:tag,published_at:at,prerelease,assets});
const asset=(name,tag,size=123)=>({name,size,browser_download_url:`https://github.com/CobbleLauncher/CobbleLauncher/releases/download/${tag}/${name}`});
function fixture(nav,options={}){
  const dom=new JSDOM(html,{url:'https://cobble.example/'}),doc=dom.window.document,downloads=setupDownloads(doc,nav,options);
  downloads.setLanguage('zh');
  const key=(element,value)=>element.dispatchEvent(new dom.window.KeyboardEvent('keydown',{key:value,bubbles:true,cancelable:true}));
  return {dom,doc,downloads,key};
}

test('desktop detection stays conservative for mobile and unknown platforms',()=>{
  for(const [nav,expected] of [
    [{userAgentData:{platform:'Windows',mobile:false}},'windows'],[{platform:'Win32'},'windows'],
    [{platform:'MacIntel'},'macos'],[{userAgent:'Mozilla/5.0 (X11; Linux x86_64)'},'linux'],
    [{platform:'Linux',userAgent:'Android'},'unknown'],[{platform:'MacIntel',maxTouchPoints:5},'unknown'],
    [{userAgent:'iPhone'},'unknown'],[{userAgentData:{platform:'Windows',mobile:true}},'unknown'],[{},'unknown'],
  ])assert.equal(detectDesktopOS(nav),expected);
  assert.equal(detectDesktopArch({platform:'Linux x86_64'}),'x64');
  assert.equal(detectDesktopArch({userAgent:'X11; Linux aarch64'}),'arm64');
  assert.equal(detectDesktopArch({platform:'Linux'}),'unknown');
});

test('selects the newest real downloadable asset independently for each OS and architecture',()=>{
  const releases=[
    {...release('draft-newest','2026-10-29T10:00:00Z',[asset('CobbleLauncher-windows-x64.zip','draft-newest')]),draft:true},
    release('macos-preview-new','2026-10-30T10:00:00Z',[asset('CobbleLauncher-macos-arm64.tar.gz','macos-preview-new'),asset('CobbleLauncher-linux-x64.tar.gz','macos-preview-new')]),
    release('windows-new','2026-10-26T10:00:00Z',[asset('CobbleLauncher-windows-x64-setup.exe','windows-new'),asset('CobbleLauncher-windows-x64.zip','windows-new'),asset('CobbleLauncher-macos-x64.tar.gz','windows-new')]),
    release('linux-internal','2026-10-28T10:00:00Z',[asset('CobbleLauncher-0.1.0-linux-x64-internal.tar.gz','linux-internal')]),
    release('old-mac','2026-10-20T10:00:00Z',[asset('CobbleLauncher-macos-arm64.tar.gz','old-mac'),asset('CobbleLauncher-macos-x64.tar.gz','old-mac')]),
  ];
  const selected=selectReleaseAssets(releases);
  assert.deepEqual(selected.map(item=>[item.os,item.arch,item.format??null,item.version]),[
    ['windows','x64','installer','windows-new'],['windows','x64','portable','windows-new'],
    ['macos','arm64',null,'macos-preview-new'],['macos','x64',null,'windows-new'],['linux','x64',null,'macos-preview-new'],
  ]);
  assert.ok(selected.every(item=>item.href?.includes('/releases/download/')));
  assert.deepEqual(selectReleaseAssets([]).map(item=>item.href),[null,null,null,null,null]);
  assert.ok(desktopReleases.find(item=>item.format==='portable').href,'the initial Windows fallback remains a real ZIP');
});

test('rejects sidecars, zero-byte files, internal packages and URLs outside the project release page',()=>{
  const candidates=[
    asset('CobbleLauncher-windows-x64.zip.sha256','v3'),
    asset('CobbleLauncher-windows-x64.zip','v2',0),
    {...asset('CobbleLauncher-windows-x64.zip','v1'),browser_download_url:'https://example.com/file.zip'},
  ];
  assert.equal(selectReleaseAssets([release('newest','2026-10-30T10:00:00Z',candidates)])[0].version,null);
});

test('Windows keeps the real first-render fallback and updates from the release API',async()=>{
  const releases=[release('v0.1.0-beta.3','2026-10-30T10:00:00Z',[asset('CobbleLauncher-windows-x64-setup.exe','v0.1.0-beta.3'),asset('CobbleLauncher-windows-x64.zip','v0.1.0-beta.3')])];
  const {dom,doc,downloads}=fixture({platform:'Win32'},{fetch:async url=>{
    assert.equal(url,releasesApi);return {ok:true,json:async()=>releases};
  }});
  assert.equal(doc.querySelector('[data-get]').href,desktopReleases.find(item=>item.format==='installer').href);
  assert.equal(doc.querySelector('[data-get]').textContent,'Windows x64 安装程序 · Beta');
  await downloads.ready;
  assert.match(doc.querySelector('[data-get]').href,/v0\.1\.0-beta\.3/);
  assert.match(doc.querySelector('[data-download-detail]').textContent,/v0\.1\.0-beta\.3/);
  dom.window.close();
});

test('stable Windows packages omit Beta while the menu retains the real release tag',async()=>{
  const releases=[release('v1.0.0','2026-10-30T10:00:00Z',[asset('CobbleLauncher-windows-x64.zip','v1.0.0')],false)];
  const {dom,doc,downloads}=fixture({platform:'Win32'},{fetch:async()=>({ok:true,json:async()=>releases})});
  await downloads.ready;
  assert.equal(doc.querySelector('[data-get]').textContent,'Windows x64');
  assert.match(doc.querySelector('.download-menu').textContent,/Windows x64 · 便携版 ZIPv1\.0\.0/);
  assert.doesNotMatch(doc.querySelector('.download-menu').textContent,/Beta/);
  dom.window.close();
});

test('published Windows installer becomes primary and the portable ZIP stays selectable',async()=>{
  const tag='v0.1.0-beta.3',releases=[release(tag,'2026-10-29T10:00:00Z',[
    asset('CobbleLauncher-windows-x64-setup.exe',tag),asset('CobbleLauncher-windows-x64.zip',tag),
  ])];
  const {dom,doc,downloads}=fixture({platform:'Win32'},{fetch:async()=>({ok:true,json:async()=>releases})});
  await downloads.ready;
  const installer=asset('CobbleLauncher-windows-x64-setup.exe',tag).browser_download_url;
  const portable=asset('CobbleLauncher-windows-x64.zip',tag).browser_download_url;
  const main=doc.querySelector('[data-get]'),menu=doc.querySelector('.download-menu');
  assert.equal(main.href,installer);
  assert.equal(main.textContent,'Windows x64 安装程序 · Beta');
  assert.match(doc.querySelector('[data-download-detail]').textContent,/v0\.1\.0-beta\.3/);
  assert.equal(menu.querySelectorAll('a[role="menuitem"]').length,5);
  assert.equal([...menu.querySelectorAll('a')].find(link=>link.textContent.includes('Windows x64 安装程序')).href,installer);
  assert.equal([...menu.querySelectorAll('a')].find(link=>link.textContent.includes('便携版 ZIP')).href,portable);
  downloads.setLanguage('en');
  assert.equal(main.textContent,'Windows x64 Installer · Beta');
  assert.match(menu.textContent,/Portable ZIP/);
  dom.window.close();
});

test('API failure uses current published Windows and macOS assets while Linux stays unpublished',async()=>{
  const {dom,doc,downloads}=fixture({platform:'Win32'},{fetch:async()=>{throw new Error('offline');}});
  await downloads.ready;
  const winSetup=desktopReleases.find(item=>item.format==='installer');
  const winZip=desktopReleases.find(item=>item.format==='portable');
  const macArm=desktopReleases.find(item=>item.os==='macos'&&item.arch==='arm64');
  const macIntel=desktopReleases.find(item=>item.os==='macos'&&item.arch==='x64');
  assert.equal(doc.querySelector('[data-get]').href,winSetup.href);
  assert.match(doc.querySelector('[data-get]').href,/v0\.1\.0-beta\.3\/CobbleLauncher-windows-x64-setup\.exe$/);
  const menu=doc.querySelector('.download-menu');
  assert.equal([...menu.querySelectorAll('a')].find(link=>link.textContent.includes('便携版 ZIP')).href,winZip.href);
  assert.equal([...menu.querySelectorAll('a')].find(link=>link.textContent.includes('macOS Apple 芯片')).href,macArm.href);
  assert.equal([...menu.querySelectorAll('a')].find(link=>link.textContent.includes('macOS Intel')).href,macIntel.href);
  assert.match(menu.textContent,/macOS Apple 芯片.*预览版/);
  assert.match(menu.textContent,/Linux x64尚未发布/);
  dom.window.close();
});

test('a stale API snapshot does not replace newer published Windows fallbacks',async()=>{
  const tag='v0.1.0-beta.1',releases=[release(tag,'2026-10-25T10:00:00Z',[
    asset('CobbleLauncher-windows-x64-setup.exe',tag),asset('CobbleLauncher-windows-x64.zip',tag),
  ])];
  const {dom,doc,downloads}=fixture({platform:'Win32'},{fetch:async()=>({ok:true,json:async()=>releases})});
  await downloads.ready;
  assert.match(doc.querySelector('[data-get]').href,/v0\.1\.0-beta\.3\/CobbleLauncher-windows-x64-setup\.exe$/);
  const windowsLinks=[...doc.querySelectorAll('.download-menu a')].filter(link=>link.textContent.includes('Windows x64'));
  assert.equal(windowsLinks.length,2);
  assert.ok(windowsLinks.every(link=>/v0\.1\.0-beta\.3/.test(link.href)));
  dom.window.close();
});

test('a newer fetched Windows ZIP is not replaced by the older installer fallback',async()=>{
  const tag='v0.1.0-beta.4',releases=[release(tag,'2026-10-30T10:00:00Z',[asset('CobbleLauncher-windows-x64.zip',tag)])];
  const {dom,doc,downloads}=fixture({platform:'Win32'},{fetch:async()=>({ok:true,json:async()=>releases})});
  await downloads.ready;
  assert.match(doc.querySelector('[data-get]').href,new RegExp(`${tag}/CobbleLauncher-windows-x64\\.zip$`));
  assert.doesNotMatch(doc.querySelector('[data-get]').href,/v0\.1\.0-beta\.3/);
  assert.doesNotMatch([...doc.querySelectorAll('.download-menu a')].map(link=>link.href).join('\n'),/v0\.1\.0-beta\.3\/CobbleLauncher-windows-x64-setup\.exe/);
  dom.window.close();
});

test('Mac defaults to Apple silicon and clearly offers Intel with preview and notarization status',async()=>{
  const releases=[release('macos-preview-42','2026-10-30T10:00:00Z',[
    asset('CobbleLauncher-macos-arm64.tar.gz','macos-preview-42'),asset('CobbleLauncher-macos-x64.tar.gz','macos-preview-42'),
  ])];
  const {dom,doc,downloads,key}=fixture({platform:'MacIntel'},{fetch:async()=>({ok:true,json:async()=>releases})});
  await downloads.ready;
  const main=doc.querySelector('[data-get]');
  const localDate=new Date('2026-10-30T10:00:00Z').toLocaleDateString('zh-CN',{year:'numeric',month:'numeric',day:'numeric'});
  assert.match(main.href,/macos-arm64\.tar\.gz$/);
  assert.equal(main.textContent,'macOS Apple 芯片 · 预览版');
  assert.doesNotMatch(main.textContent,/macos-preview-42/);
  assert.equal(doc.querySelector('[data-download-detail]').textContent,`macOS Apple 芯片 · ${localDate} · 预览版`);
  assert.doesNotMatch(doc.querySelector('[data-download-detail]').textContent,/macos-preview-42/);
  assert.equal(main.hasAttribute('aria-haspopup'),false);
  const menu=doc.querySelector('.download-menu');
  assert.match(menu.textContent,/macOS Apple 芯片/);
  assert.match(menu.textContent,/macOS Intel/);
  assert.match(menu.textContent,new RegExp(`${localDate} · 预览版`));
  assert.doesNotMatch(menu.textContent,/macos-preview-42/);
  assert.equal(menu.querySelectorAll('a[role="menuitem"]').length,5);
  assert.equal(menu.querySelectorAll('.download-menu-note').length,1);
  assert.equal(menu.querySelector('.download-menu-note').textContent,'macOS 预览包未公证，尚未实机验收。');
  doc.querySelector('.download-toggle').click();key(doc.activeElement,'End');
  assert.equal(doc.activeElement,menu.querySelectorAll('a')[4]);
  key(doc.activeElement,'Home');assert.equal(doc.activeElement,menu.querySelectorAll('a')[0]);
  assert.equal([...menu.querySelectorAll('a')].find(a=>a.textContent.includes('macOS Intel')).href,
    'https://github.com/CobbleLauncher/CobbleLauncher/releases/download/macos-preview-42/CobbleLauncher-macos-x64.tar.gz');
  downloads.setLanguage('en');
  assert.equal(main.textContent,'macOS Apple silicon · Preview');
  assert.doesNotMatch(menu.textContent,/macos-preview-42/);
  assert.match(menu.querySelector('.download-menu-note').textContent,/not notarized or hardware-tested/);
  dom.window.close();
});

test('an undated legacy Mac cache retains the newer verified public fallback',async()=>{
  const tag='macos-preview-99',file='CobbleLauncher-macos-arm64.tar.gz';
  const items=[
    {os:'windows',arch:'x64',file:'CobbleLauncher-windows-x64.zip',version:'v0.1.0-beta.3',prerelease:true,href:desktopReleases.find(item=>item.format==='portable').href},
    {os:'macos',arch:'arm64',file,version:tag,prerelease:true,href:asset(file,tag).browser_download_url},
  ];
  const values=new Map([[releaseCacheKey,JSON.stringify({updatedAt:1000,items})]]);
  const storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};
  const {dom,doc,downloads}=fixture({platform:'MacIntel'},{storage,now:()=>1001,fetch:()=>{throw new Error('fresh cache should be used');}});
  await downloads.ready;
  const mac=[...doc.querySelectorAll('.download-menu a')].find(link=>link.textContent.includes('Apple 芯片'));
  assert.equal(mac.href,desktopReleases.find(item=>item.os==='macos'&&item.arch==='arm64').href);
  assert.match(mac.querySelector('small').textContent,/预览版$/);
  assert.doesNotMatch(mac.textContent,/macos-preview-99/);
  assert.equal(doc.querySelectorAll('.download-menu-note').length,1);
  dom.window.close();
});

test('Linux is recognized, but no direct package link is shown without a matching asset',async()=>{
  const {dom,doc,downloads}=fixture({userAgent:'Mozilla/5.0 (X11; Linux x86_64)'},{fetch:async()=>({ok:true,json:async()=>[]})});
  await downloads.ready;
  const main=doc.querySelector('[data-get]');
  assert.equal(main.href,releasesPage);
  assert.equal(main.getAttribute('aria-haspopup'),'menu');
  assert.equal(main.getAttribute('aria-controls'),'download-systems');
  assert.match(main.textContent,/Linux x64 · 尚未发布/);
  assert.equal(doc.querySelector('.download-menu a[href$="CobbleLauncher-linux-x64.tar.gz"]'),null);
  assert.match(doc.querySelector('.download-menu').textContent,/Linux x64尚未发布/);
  dom.window.close();
});

test('Linux ARM browsers do not receive an x64 download link',async()=>{
  const {dom,doc,downloads}=fixture({userAgent:'Mozilla/5.0 (X11; Linux aarch64)'},{fetch:async()=>({ok:true,json:async()=>[
    release('linux-x64','2026-10-30T10:00:00Z',[asset('CobbleLauncher-linux-x64.tar.gz','linux-x64')]),
  ]})});
  await downloads.ready;
  assert.equal(doc.querySelector('[data-get]').href,releasesPage);
  assert.match(doc.querySelector('[data-get]').textContent,/选择桌面系统/);
  assert.match(doc.querySelector('.download-menu').textContent,/Linux x64/);
  dom.window.close();
});

test('release cache avoids repeat API calls and stale entries survive network failure',async()=>{
  const values=new Map(),storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};
  let calls=0,now=1_000;
  const releases=[release('macos-preview-42','2026-10-30T10:00:00Z',[asset('CobbleLauncher-macos-arm64.tar.gz','macos-preview-42')])];
  const first=fixture({platform:'MacIntel'},{storage,now:()=>now,fetch:async()=>{calls++;return {ok:true,json:async()=>releases};}});
  await first.downloads.ready;first.dom.window.close();
  assert.equal(calls,1);assert.ok(values.has(releaseCacheKey));
  now+=5*60*1000;
  const second=fixture({platform:'MacIntel'},{storage,now:()=>now,fetch:async()=>{calls++;throw new Error('offline');}});
  await second.downloads.ready;
  assert.equal(calls,1);
  assert.match(second.doc.querySelector('[data-get]').href,/macos-arm64/);
  second.dom.window.close();
  now+=releaseCacheTtl+1;
  const stale=fixture({platform:'MacIntel'},{storage,now:()=>now,fetch:async()=>{calls++;throw new Error('rate limited');}});
  await stale.downloads.ready;
  assert.equal(calls,2);
  assert.match(stale.doc.querySelector('[data-get]').href,/macos-arm64/);
  stale.dom.window.close();
});

test('a slow GitHub API request times out without blocking the Windows fallback',async()=>{
  const {dom,doc,downloads}=fixture({platform:'Win32'},{timeoutMs:10,fetch:(_url,{signal})=>new Promise((_,reject)=>{
    signal.addEventListener('abort',()=>reject(new Error('request aborted')), {once:true});
  })});
  assert.equal(doc.querySelector('[data-get]').textContent,'Windows x64 安装程序 · Beta');
  await downloads.ready;
  assert.equal(doc.querySelector('[data-get]').href,desktopReleases.find(item=>item.format==='installer').href);
  assert.equal(doc.querySelector('[data-get]').textContent,'Windows x64 安装程序 · Beta');
  dom.window.close();
});

test('unavailable systems and unknown/mobile devices keep an accessible shared menu',()=>{
  for(const nav of [{platform:'Linux x86_64'},{userAgent:'Android Mobile'}]){
    const {dom,doc,key}=fixture(nav),main=doc.querySelector('.arrival [data-get]'),toggle=doc.querySelector('.arrival .download-toggle'),panel=doc.querySelector('.download-menu');
    assert.equal(main.tagName,'A');main.click();assert.equal(panel.hidden,false);assert.equal(doc.activeElement,panel.children[0]);
    key(doc.activeElement,'End');assert.equal(doc.activeElement,panel.querySelectorAll('a')[4]);key(doc.activeElement,'ArrowDown');assert.equal(doc.activeElement,panel.querySelectorAll('a')[0]);
    key(doc.activeElement,'Escape');assert.equal(panel.hidden,true);assert.equal(doc.activeElement,main);
    key(toggle,'ArrowUp');assert.equal(doc.activeElement,panel.querySelectorAll('a')[4]);key(doc.activeElement,'Home');assert.equal(doc.activeElement,panel.querySelectorAll('a')[0]);dom.window.close();
  }
});

test('outside click, focus exit, selection and language change close the shared menu',()=>{
  const {dom,doc,downloads}=fixture({platform:'Win32'}),toggle=doc.querySelector('.arrival .download-toggle'),panel=doc.querySelector('.download-menu');
  toggle.click();doc.body.dispatchEvent(new dom.window.Event('pointerdown',{bubbles:true}));assert.equal(panel.hidden,true);
  toggle.click();doc.querySelector('#language').focus();assert.equal(panel.hidden,true);
  toggle.click();panel.children[0].addEventListener('click',event=>event.preventDefault());panel.children[0].click();assert.equal(panel.hidden,true);
  toggle.click();downloads.setLanguage('en');assert.equal(panel.hidden,true);assert.equal(panel.getAttribute('aria-label'),'Other systems');
  assert.match(panel.children[0].textContent,/Windows x64.*v0\.1\.0-beta\.3/);dom.window.close();
});

test('download copy no longer says all systems are under validation or unavailable',()=>{
  assert.match(copy,/platform:'Windows x64<br>macOS preview · Linux not released'/);
  assert.match(copy,/downloadBody:'The download menu checks published release assets/);
  assert.doesNotMatch(copy,/macOS \/ Linux under validation|No public application package has been released yet/);
});
