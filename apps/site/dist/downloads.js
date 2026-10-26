// SPDX-License-Identifier: MIT
export const releasesPage='https://github.com/CobbleLauncher/CobbleLauncher/releases';
export const desktopReleases=Object.freeze([
  {os:'windows',name:'Windows x64',version:'Beta · ZIP',href:'https://github.com/CobbleLauncher/CobbleLauncher/releases/download/v0.1.0-beta.1/CobbleLauncher-windows-x64.zip'},
  {os:'macos',name:'macOS',href:null},
  {os:'linux',name:'Linux',href:null},
]);

// Detection only selects a label/link; it never invents an architecture or package.
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

const words={
  zh:{choose:'选择桌面系统',other:'其他系统',unreleased:'尚未发布',versions:'查看版本页',detail:'Windows x64 · Beta 便携版',available:'Windows x64 Beta 已发布',download:'下载 Windows x64 Beta 便携版'},
  en:{choose:'Choose desktop OS',other:'Other systems',unreleased:'Not released yet',versions:'View releases',detail:'Windows x64 · Portable Beta',available:'Windows x64 Beta available',download:'Download Windows x64 portable Beta'},
};

export function setupDownloads(doc,nav={}){
  const os=detectDesktopOS(nav),selected=desktopReleases.find(item=>item.os===os),controls=[];
  const panel=doc.createElement('div');panel.id='download-systems';panel.className='download-menu';panel.setAttribute('role','menu');panel.hidden=true;doc.body.append(panel);
  let language='zh',opener=null;
  function close(restore=false){
    panel.hidden=true;for(const {toggle,main} of controls){toggle.setAttribute('aria-expanded','false');if(main.tagName==='BUTTON')main.setAttribute('aria-expanded','false');}
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
  for(const original of [...doc.querySelectorAll('[data-get]')]){
    const wrap=doc.createElement('span');wrap.className='download-control';
    if(original.closest('.site-header'))wrap.classList.add('download-compact');
    if(original.closest('.chapter-bottom'))wrap.classList.add('download-footer');
    const main=doc.createElement(selected?'a':'button');main.className=original.className;main.dataset.get='';
    if(selected)main.href=selected.href??releasesPage;
    else{main.type='button';main.setAttribute('aria-haspopup','menu');main.setAttribute('aria-controls',panel.id);main.setAttribute('aria-expanded','false');main.addEventListener('click',()=>panel.hidden||opener!==main?open(main,true):close());}
    const toggle=doc.createElement('button');toggle.type='button';toggle.className='download-toggle';toggle.setAttribute('aria-haspopup','menu');toggle.setAttribute('aria-controls',panel.id);toggle.setAttribute('aria-expanded','false');
    toggle.innerHTML='<svg viewBox="0 0 20 20" class="icon" aria-hidden="true"><path d="m5 8 5 5 5-5"/></svg>';
    toggle.addEventListener('click',()=>panel.hidden||opener!==toggle?open(toggle,true):close());
    for(const trigger of main.tagName==='BUTTON'?[main,toggle]:[toggle])trigger.addEventListener('keydown',event=>{
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
  doc.addEventListener('focusin',event=>{if(!panel.hidden&&!panel.contains(event.target)&&event.target!==opener)close();});
  doc.defaultView.addEventListener('resize',()=>close());doc.defaultView.addEventListener('scroll',()=>close(),{passive:true});
  return {setLanguage(value){
    close();language=value==='en'?'en':'zh';const text=words[language];panel.setAttribute('aria-label',text.other);
    panel.replaceChildren(...desktopReleases.map(release=>{
      const link=doc.createElement('a');link.href=release.href??releasesPage;link.setAttribute('role','menuitem');link.tabIndex=-1;
      const title=doc.createElement('span');title.textContent=release.name;const detail=doc.createElement('small');detail.textContent=release.href?release.version:`${text.unreleased} · ${text.versions}`;link.append(title,detail);return link;
    }));
    for(const {main,toggle} of controls){
      main.textContent=selected?(selected.href?'Windows x64 Beta':`${selected.name} · ${text.unreleased}`):text.choose;
      main.setAttribute('aria-label',selected?.href?text.download:main.textContent+(selected?` · ${text.versions}`:''));
      toggle.setAttribute('aria-label',text.other);toggle.title=text.other;
    }
    for(const detail of doc.querySelectorAll('[data-download-detail]'))detail.textContent=os==='windows'?text.detail:text.available;
  }};
}
