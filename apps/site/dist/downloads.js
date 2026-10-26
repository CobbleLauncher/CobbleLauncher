// SPDX-License-Identifier: MIT
export const releasesPage='https://github.com/CobbleLauncher/CobbleLauncher/releases';
export const releasesApi='https://api.github.com/repos/CobbleLauncher/CobbleLauncher/releases?per_page=50';
export const releaseCacheKey='cobble-site-desktop-releases-v1';
export const releaseCacheTtl=30*60*1000;

const specs=Object.freeze([
  {os:'windows',arch:'x64',name:'Windows x64',file:'CobbleLauncher-windows-x64.zip'},
  {os:'macos',arch:'arm64',name:'macOS Apple silicon',file:'CobbleLauncher-macos-arm64.tar.gz'},
  {os:'macos',arch:'x64',name:'macOS Intel',file:'CobbleLauncher-macos-x64.tar.gz'},
  {os:'linux',arch:'x64',name:'Linux x64',file:'CobbleLauncher-linux-x64.tar.gz'},
]);
const windowsFallback={...specs[0],version:'v0.1.0-beta.1',prerelease:true,href:'https://github.com/CobbleLauncher/CobbleLauncher/releases/download/v0.1.0-beta.1/CobbleLauncher-windows-x64.zip'};
export const desktopReleases=Object.freeze([
  windowsFallback,
  ...specs.slice(1).map(spec=>({...spec,version:null,prerelease:false,href:null})),
]);

// Detection chooses an OS only. Mac architecture is intentionally a user choice.
export function detectDesktopOS(nav={}){
  const ua=nav.userAgent??'',platform=nav.userAgentData?.platform??nav.platform??'';
  if(nav.userAgentData?.mobile||/Android|iOS|iPhone|iPad|iPod|Mobile/i.test(`${platform} ${ua}`)||(/Mac/i.test(platform)&&nav.maxTouchPoints>1))return 'unknown';
  for(const source of [platform,ua]){
    if(/Windows|Win32|Win64/i.test(source))return 'windows';
    if(/macOS|MacIntel|Macintosh|MacPPC/i.test(source))return 'macos';
    if(/Linux|X11/i.test(source))return 'linux';
  }
  return 'unknown';
}
export function detectDesktopArch(nav={}){
  const source=`${nav.userAgentData?.architecture??''} ${nav.platform??''} ${nav.userAgent??''}`;
  if(/arm64|aarch64/i.test(source))return 'arm64';
  if(/x86_64|amd64|x64/i.test(source))return 'x64';
  return 'unknown';
}

const words={
  zh:{choose:'选择桌面系统',other:'其他系统',unreleased:'尚未发布',versions:'查看版本页',download:'下载',preview:'预览版',beta:'Beta',
    windows:'Windows x64',macArm:'macOS Apple 芯片',macIntel:'macOS Intel',linux:'Linux x64',portable:'便携版 ZIP',macNote:'macOS 预览包未公证，尚未实机验收。'},
  en:{choose:'Choose desktop OS',other:'Other systems',unreleased:'Not released yet',versions:'View releases',download:'Download',preview:'Preview',beta:'Beta',
    windows:'Windows x64',macArm:'macOS Apple silicon',macIntel:'macOS Intel',linux:'Linux x64',portable:'Portable ZIP',macNote:'macOS previews are not notarized or hardware-tested.'},
};

function nameOf(item,text){
  if(item.os==='windows')return text.windows;
  if(item.os==='linux')return text.linux;
  return item.arch==='arm64'?text.macArm:text.macIntel;
}
function validReleaseUrl(value,file){
  try{
    const url=new URL(value);
    return url.origin==='https://github.com'
      && url.pathname.startsWith('/CobbleLauncher/CobbleLauncher/releases/download/')
      && decodeURIComponent(url.pathname.split('/').at(-1))===file;
  }catch{return false;}
}
function ordered(releases){
  return [...releases].filter(item=>item&&item.draft!==true&&typeof item.tag_name==='string')
    .sort((a,b)=>(Date.parse(b.published_at??b.created_at??'')||0)-(Date.parse(a.published_at??a.created_at??'')||0));
}
export function selectReleaseAssets(releases){
  const sorted=ordered(Array.isArray(releases)?releases:[]);
  return specs.map(spec=>{
    for(const release of sorted){
      const asset=(Array.isArray(release.assets)?release.assets:[]).find(candidate=>candidate?.name===spec.file
        && Number(candidate.size)>0 && validReleaseUrl(candidate.browser_download_url,spec.file));
      if(asset)return {...spec,version:release.tag_name,publishedAt:release.published_at??null,prerelease:release.prerelease===true,href:asset.browser_download_url};
    }
    return {...spec,version:null,prerelease:false,href:null};
  });
}
function mergeFallback(selected){
  return selected.map(item=>item.href?item:item.os==='windows'?windowsFallback:{...item,version:null,prerelease:false,href:null});
}
function validCacheItems(items){
  if(!Array.isArray(items))return null;
  const byFile=new Map(items.filter(item=>item&&typeof item.file==='string').map(item=>[item.file,item]));
  const normalized=specs.map(spec=>{
    const item=byFile.get(spec.file);
    if(item&&typeof item.version==='string'&&validReleaseUrl(item.href,spec.file))
      return {...spec,version:item.version,publishedAt:typeof item.publishedAt==='string'?item.publishedAt:null,prerelease:item.prerelease===true,href:item.href};
    return {...spec,version:null,prerelease:false,href:null};
  });
  return mergeFallback(normalized);
}
function storedCache(storage){
  try{
    const value=JSON.parse(storage?.getItem(releaseCacheKey)??'null');
    const items=validCacheItems(value?.items);
    return items?{items,updatedAt:Number(value.updatedAt)||0}:null;
  }catch{return null;}
}
function saveCache(storage,items,now){
  try{storage?.setItem(releaseCacheKey,JSON.stringify({updatedAt:now,items}));}catch{/* Cache is optional. */}
}

export function setupDownloads(doc,nav={},options={}){
  const os=detectDesktopOS(nav),controls=[];
  const panel=doc.createElement('div');panel.id='download-systems';panel.className='download-menu';panel.setAttribute('role','menu');panel.hidden=true;doc.body.append(panel);
  let language='zh',opener=null,current=desktopReleases;
  const now=options.now??Date.now;
  const fetcher=options.fetch??doc.defaultView.fetch?.bind(doc.defaultView);
  let storage=options.storage;
  if(storage===undefined){try{storage=doc.defaultView.localStorage;}catch{storage=null;}}
  function defaultEntry(){
    if(os==='unknown')return null;
    const arch=os==='macos'?'arm64':os==='linux'?detectDesktopArch(nav):'x64';
    if(arch==='unknown')return null;
    return current.find(item=>item.os===os&&item.arch===arch)??null;
  }
  function close(restore=false){
    panel.hidden=true;for(const {toggle,main} of controls){toggle.setAttribute('aria-expanded','false');main.setAttribute('aria-expanded','false');}
    if(restore)opener?.focus();opener=null;
  }
  function position(){
    if(!opener)return;
    const rect=opener.parentElement.getBoundingClientRect(),view=doc.defaultView,width=Math.min(280,view.innerWidth-32);
    panel.style.width=`${width}px`;panel.style.left=`${Math.max(16,Math.min(rect.left,view.innerWidth-width-16))}px`;
    const height=panel.getBoundingClientRect().height;
    panel.style.top=`${rect.bottom+height+12<=view.innerHeight?rect.bottom+8:Math.max(12,rect.top-height-8)}px`;
  }
  function open(trigger,focus=false,last=false){
    close();opener=trigger;panel.hidden=false;trigger.setAttribute('aria-expanded','true');position();
    if(focus){const items=panel.querySelectorAll('a');items[last?items.length-1:0]?.focus();}
  }
  function detailOf(item,text){
    if(!item.href)return `${text.unreleased} · ${text.versions}`;
    if(item.os==='macos'){
      const timestamp=Date.parse(item.publishedAt??'');
      const date=Number.isFinite(timestamp)?new Date(timestamp).toLocaleDateString(language==='en'?'en-US':'zh-CN',{year:'numeric',month:'numeric',day:'numeric'}):null;
      return date?`${date} · ${text.preview}`:text.preview;
    }
    if(item.os==='windows')return `${item.version}${item.prerelease?` · ${text.beta}`:''} · ${text.portable}`;
    return item.version;
  }
  function renderMenu(text){
    panel.setAttribute('aria-label',text.other);
    const entries=current.map(item=>{
      const link=doc.createElement('a');link.href=item.href??releasesPage;link.setAttribute('role','menuitem');link.tabIndex=-1;
      const title=doc.createElement('span');title.textContent=nameOf(item,text);
      const detail=doc.createElement('small');detail.textContent=detailOf(item,text);link.append(title,detail);return link;
    });
    if(current.some(item=>item.os==='macos'&&item.href)){
      const note=doc.createElement('p');note.className='download-menu-note';note.setAttribute('role','note');note.textContent=text.macNote;entries.push(note);
    }
    panel.replaceChildren(...entries);
  }
  function renderControls(text){
    const selected=defaultEntry();
    for(const {main,toggle} of controls){
      main.href=selected?.href??releasesPage;
      const label=selected?.href
        ? (selected.os==='windows'?`${text.windows}${selected.prerelease?` ${text.beta}`:''}`:selected.os==='macos'?`${nameOf(selected,text)} · ${text.preview}`:nameOf(selected,text))
        : selected?`${nameOf(selected,text)} · ${text.unreleased}`:text.choose;
      main.textContent=label;
      main.setAttribute('aria-label',selected?.href?`${text.download} ${label}`:selected?`${label} · ${text.versions}`:text.choose);
      if(selected?.href){main.removeAttribute('aria-haspopup');main.removeAttribute('aria-controls');main.removeAttribute('aria-expanded');}
      else{main.setAttribute('aria-haspopup','menu');main.setAttribute('aria-controls',panel.id);main.setAttribute('aria-expanded',String(opener===main));}
      toggle.setAttribute('aria-label',text.other);toggle.title=text.other;
    }
    const detail=selected?.href?`${nameOf(selected,text)} · ${selected.os==='macos'?detailOf(selected,text):selected.version}`
      :selected?`${nameOf(selected,text)} · ${text.unreleased}`:text.choose;
    for(const node of doc.querySelectorAll('[data-download-detail]'))node.textContent=detail;
  }
  function render(){
    const focused=panel.hidden?-1:[...panel.querySelectorAll('a')].indexOf(doc.activeElement);
    const text=words[language];renderMenu(text);renderControls(text);
    if(!panel.hidden){position();if(focused>=0)panel.querySelectorAll('a')[Math.min(focused,panel.children.length-1)]?.focus();}
  }
  for(const original of [...doc.querySelectorAll('[data-get]')]){
    const wrap=doc.createElement('span');wrap.className='download-control';
    if(original.closest('.site-header'))wrap.classList.add('download-compact');
    if(original.closest('.chapter-bottom'))wrap.classList.add('download-footer');
    const main=doc.createElement('a');main.className=original.className;main.dataset.get='';main.href=releasesPage;
    main.addEventListener('click',event=>{if(!defaultEntry()?.href){event.preventDefault();open(main,true);}});
    const toggle=doc.createElement('button');toggle.type='button';toggle.className='download-toggle';toggle.setAttribute('aria-haspopup','menu');toggle.setAttribute('aria-controls',panel.id);toggle.setAttribute('aria-expanded','false');
    toggle.innerHTML='<svg viewBox="0 0 20 20" class="icon" aria-hidden="true"><path d="m5 8 5 5 5-5"/></svg>';
    toggle.addEventListener('click',()=>panel.hidden||opener!==toggle?open(toggle,true):close());
    for(const trigger of [main,toggle])trigger.addEventListener('keydown',event=>{
      if(event.key==='ArrowDown'||event.key==='ArrowUp'){event.preventDefault();open(trigger,true,event.key==='ArrowUp');}
    });
    original.replaceWith(wrap);wrap.append(main,toggle);controls.push({main,toggle});
  }
  panel.addEventListener('click',event=>{if(event.target.closest('a'))close();});
  panel.addEventListener('keydown',event=>{
    const items=[...panel.querySelectorAll('a')],at=items.indexOf(doc.activeElement);
    if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){
      event.preventDefault();items[event.key==='Home'?0:event.key==='End'?items.length-1:(at+(event.key==='ArrowDown'?1:-1)+items.length)%items.length].focus();
    }
  });
  doc.addEventListener('keydown',event=>{if(event.key==='Escape'&&!panel.hidden){event.preventDefault();close(true);}});
  doc.addEventListener('pointerdown',event=>{if(!panel.hidden&&!panel.contains(event.target)&&!controls.some(({main,toggle})=>main.contains(event.target)||toggle.contains(event.target)))close();});
  doc.addEventListener('focusin',event=>{if(!panel.hidden&&!panel.contains(event.target)&&!controls.some(({main,toggle})=>main===event.target||toggle===event.target))close();});
  doc.defaultView.addEventListener('resize',()=>close());doc.defaultView.addEventListener('scroll',()=>close(),{passive:true});
  function setLanguage(value){close();language=value==='en'?'en':'zh';render();}
  const initial=storedCache(storage);
  if(initial){current=initial.items;render();}
  async function refresh(){
    if(!fetcher)return current;
    const Controller=options.AbortController??doc.defaultView.AbortController??globalThis.AbortController;
    const controller=Controller?new Controller():null;
    const timeoutMs=options.timeoutMs??5000;
    const timeout=controller?doc.defaultView.setTimeout(()=>controller.abort(),timeoutMs):null;
    try{
      const response=await fetcher(releasesApi,{headers:{Accept:'application/vnd.github+json'},...(controller?{signal:controller.signal}:{})});
      if(!response.ok)throw new Error(`GitHub releases request failed: ${response.status}`);
      const value=await response.json();
      if(!Array.isArray(value))throw new Error('Unexpected GitHub releases response');
      current=mergeFallback(selectReleaseAssets(value));saveCache(storage,current,now());render();
    }catch{
      // Keep the valid Windows fallback or the last cached real assets.
      render();
    }finally{
      if(timeout!==null)doc.defaultView.clearTimeout(timeout);
    }
    return current;
  }
  const age=initial?now()-initial.updatedAt:Infinity;
  const ready=initial&&age>=0&&age<releaseCacheTtl?Promise.resolve(current):refresh();
  render();
  return {setLanguage,ready,refresh};
}
