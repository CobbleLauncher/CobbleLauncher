// SPDX-License-Identifier: MIT
import { createTerrain } from './terrain.js';
import { english, labels } from './copy.js';
import { renderProduct, renderTool } from './demos.js';

const state={lang:'zh',feature:'play',tool:'geometry',radius:9,location:'village',running:false};
const product=document.querySelector('#product-content');
const tool=document.querySelector('#tool-panel');
const terrain=createTerrain(document.querySelector('#terrain'));
const translations=[...document.querySelectorAll('[data-i18n]')].map(element=>({element,zh:element.innerHTML,key:element.dataset.i18n}));
const reduced=matchMedia('(prefers-reduced-motion: reduce)');
function renderDemos(){renderProduct(product,state.feature,state.lang,state);renderTool(tool,state.tool,state.lang,state);}
function store(key,value){try{localStorage.setItem(`cobble-site-${key}`,value);}catch{/* Optional presentation preferences. */}}
function read(key){try{return localStorage.getItem(`cobble-site-${key}`);}catch{return null;}}
function language(lang){
  state.lang=lang;
  document.querySelectorAll('[data-page]').forEach(el=>{el.href=`./${el.dataset.page}${lang==='en'?'.en':''}.html`;});
  document.documentElement.lang=lang==='zh'?'zh-CN':'en';
  translations.forEach(({element,zh,key})=>{element.innerHTML=lang==='zh'?zh:(english[key]??zh);});
  document.querySelectorAll('[data-label]').forEach(el=>el.setAttribute('aria-label',labels[lang][el.dataset.label]));
  const button=document.querySelector('#language');button.textContent=lang==='zh'?'EN':'中';button.setAttribute('aria-label',lang==='zh'?'Switch to English':'切换为中文');
  document.title=lang==='zh'?'圆石 Cobble Launcher — 少一点折腾，多一点世界。':'Cobble Launcher — Less friction. More world.';
  renderDemos();store('language',lang);
}
function theme(value){document.documentElement.dataset.theme=value;document.querySelector('#theme').setAttribute('aria-pressed',String(value==='dark'));store('theme',value);terrain.redraw();}
document.querySelector('#language').addEventListener('click',()=>language(state.lang==='zh'?'en':'zh'));
document.querySelector('#theme').addEventListener('click',()=>{
  const next=document.documentElement.dataset.theme==='dark'?'light':'dark';
  if(document.startViewTransition&&!reduced.matches)document.startViewTransition(()=>theme(next));else theme(next);
});
theme(read('theme')??(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'));
language(read('language')==='en'?'en':'zh');

document.querySelectorAll('[data-biome]').forEach(button=>button.addEventListener('click',()=>{
  document.querySelectorAll('[data-biome]').forEach(b=>b.setAttribute('aria-pressed',String(b===button)));terrain.setBiome(button.dataset.biome);
}));
document.querySelector('#rotate').addEventListener('click',()=>{const turn=terrain.rotate();document.querySelector('#terrain-coordinate').textContent=`${['NE','SE','SW','NW'][turn]} / ${turn*90}°`;});

function tabGroup(selector,onSelect,vertical=false){
  const buttons=[...document.querySelectorAll(selector)];
  function select(button){buttons.forEach(b=>{b.setAttribute('aria-selected',String(b===button));b.tabIndex=b===button?0:-1;});onSelect(button);}
  buttons.forEach((button,index)=>{
    button.addEventListener('click',()=>select(button));
    button.addEventListener('keydown',event=>{
      let next;
      if(event.key==='Home')next=0;else if(event.key==='End')next=buttons.length-1;
      else if(event.key==='ArrowRight'||(vertical&&event.key==='ArrowDown'))next=(index+1)%buttons.length;
      else if(event.key==='ArrowLeft'||(vertical&&event.key==='ArrowUp'))next=(index+buttons.length-1)%buttons.length;
      if(next!==undefined){event.preventDefault();select(buttons[next]);buttons[next].focus();}
    });
  });
}
tabGroup('[data-feature]',button=>{state.feature=button.dataset.feature;document.querySelector('#product-demo').setAttribute('aria-labelledby',button.id);renderProduct(product,state.feature,state.lang,state);},true);
tabGroup('[data-tool]',button=>{state.tool=button.dataset.tool;tool.setAttribute('aria-labelledby',button.id);renderTool(tool,state.tool,state.lang,state);});

const dialog=document.querySelector('#download-dialog');
let trigger;
document.querySelectorAll('[data-get]').forEach(button=>button.addEventListener('click',()=>{trigger=button;dialog.showModal();document.body.style.overflow='hidden';}));
document.querySelector('[data-close]').addEventListener('click',()=>dialog.close());
dialog.addEventListener('click',event=>{if(event.target!==dialog)return;const r=dialog.getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)dialog.close();});
dialog.addEventListener('close',()=>{document.body.style.overflow='';trigger?.focus();});

// Finite entrance transitions; no permanent animation loop or scroll hijacking.
if(!reduced.matches){
  const observer=new IntersectionObserver(entries=>entries.forEach(entry=>{if(entry.isIntersecting){entry.target.classList.add('visible');observer.unobserve(entry.target);}}),{threshold:.08});
  document.querySelectorAll('.section-heading,.experience-layout,.principle-grid,.closing-row').forEach(el=>{el.classList.add('reveal');observer.observe(el);});
}
