// SPDX-License-Identifier: MIT
import { createTerrain } from './terrain.js';
import { english, labels } from './copy.js';
import { renderCommunity } from './community.js';
import { setupDownloads } from './downloads.js';
const downloads=setupDownloads(document,navigator);
const root=document.documentElement, reduced=matchMedia('(prefers-reduced-motion: reduce)');
const terrain=createTerrain(document.querySelector('#terrain'));
const translations=[...document.querySelectorAll('[data-i18n]')].map(element=>({element,zh:element.innerHTML,key:element.dataset.i18n}));
let lang='zh';
function store(key,value){try{localStorage.setItem(`cobble-site-${key}`,value);}catch{/* Optional presentation preferences. */}}
function read(key){try{return localStorage.getItem(`cobble-site-${key}`);}catch{return null;}}
function language(value){
  lang=value;root.lang=value==='zh'?'zh-CN':'en';
  translations.forEach(({element,zh,key})=>{element.innerHTML=value==='zh'?zh:(english[key]??zh);});
  document.querySelectorAll('[data-page]').forEach(element=>{element.href=`./${element.dataset.page}${value==='en'?'.en':''}.html`;});
  document.querySelectorAll('[data-label]').forEach(element=>element.setAttribute('aria-label',labels[value][element.dataset.label]));
  const button=document.querySelector('#language');button.textContent=value==='zh'?'EN':'中';button.setAttribute('aria-label',value==='zh'?'Switch to English':'切换为中文');
  document.title=value==='zh'?'圆石 Cobble Launcher — Minecraft Java 启动器':'Cobble Launcher — Minecraft Java Launcher';
  renderCommunity(document.querySelector('#community-grid'),value);downloads.setLanguage(value);store('language',value);
}
function theme(value){root.dataset.theme=value;document.querySelector('#theme').setAttribute('aria-pressed',String(value==='dark'));store('theme',value);terrain.redraw();}
document.querySelector('#language').addEventListener('click',()=>language(lang==='zh'?'en':'zh'));
document.querySelector('#theme').addEventListener('click',()=>theme(root.dataset.theme==='dark'?'light':'dark'));
theme(read('theme')??(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'));
language(read('language')==='en'?'en':'zh');
document.querySelectorAll('[data-biome]').forEach(button=>button.addEventListener('click',()=>{
  document.querySelectorAll('[data-biome]').forEach(item=>item.setAttribute('aria-pressed',String(item===button)));terrain.setBiome(button.dataset.biome);
}));
document.querySelector('#rotate').addEventListener('click',()=>{
  terrain.rotate();document.querySelector('#announcement').textContent=lang==='zh'?'岛屿已旋转。':'Island rotated.';
});
// Only visible chapters update, once per scroll frame. No wheel interception or idle loop.
const chapters=[...document.querySelectorAll('.chapter')],visible=new Set(),wall=document.querySelector('#tool-wall');
let frame=0,manualWall=false;
function drawScroll(){
  frame=0;if(reduced.matches||document.hidden)return;
  const height=innerHeight;
  for(const chapter of visible){
    const rect=chapter.getBoundingClientRect();
    const travel=Math.max(-height,Math.min(height,-rect.top));
    chapter.querySelectorAll('[data-depth]').forEach(element=>{element.style.transform=`translate3d(0,${travel*Number(element.dataset.depth)}px,0)`;});
    if(chapter.id==='community'&&!manualWall&&!chapter.contains(document.activeElement)){
      const progress=Math.max(0,Math.min(1,(height-rect.top)/(height+rect.height)));
      wall.scrollLeft=(wall.scrollWidth-wall.clientWidth)*progress;
    }
    if(chapter.id==='together')chapter.querySelector('.network-travel').style.strokeDashoffset=String(travel*-.3);
  }
}
function schedule(){if(!frame&&!reduced.matches&&!document.hidden&&visible.size)frame=requestAnimationFrame(drawScroll);}
const observer=new IntersectionObserver(entries=>{
  for(const entry of entries){if(entry.isIntersecting)visible.add(entry.target);else {visible.delete(entry.target);if(entry.target.id==='community')manualWall=false;}}
  schedule();
});chapters.forEach(chapter=>observer.observe(chapter));
wall.addEventListener('pointerdown',()=>{manualWall=true;},{passive:true});wall.addEventListener('focusin',()=>{manualWall=true;});
wall.addEventListener('wheel',event=>{if(Math.abs(event.deltaX)>0)manualWall=true;},{passive:true});
addEventListener('scroll',schedule,{passive:true});addEventListener('resize',schedule,{passive:true});
document.addEventListener('visibilitychange',()=>{if(document.hidden){cancelAnimationFrame(frame);frame=0;}else schedule();});
reduced.addEventListener('change',()=>{
  if(reduced.matches){cancelAnimationFrame(frame);frame=0;document.querySelectorAll('[data-depth]').forEach(element=>element.style.removeProperty('transform'));document.querySelector('.network-travel').style.removeProperty('stroke-dashoffset');}
  else schedule();
});
