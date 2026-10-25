// SPDX-License-Identifier: MIT
const root=document.documentElement;
try{root.dataset.theme=localStorage.getItem('cobble-site-theme')??(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light');}catch{}
const button=document.querySelector('[data-theme-toggle]');
function sync(){button?.setAttribute('aria-pressed',String(root.dataset.theme==='dark'));}
sync();
button?.addEventListener('click',()=>{root.dataset.theme=root.dataset.theme==='dark'?'light':'dark';sync();try{localStorage.setItem('cobble-site-theme',root.dataset.theme);}catch{}});
document.querySelector('[data-language-link]')?.addEventListener('click',event=>{try{localStorage.setItem('cobble-site-language',event.currentTarget.dataset.languageLink);}catch{}});
try{localStorage.setItem('cobble-site-language',root.lang==='en'?'en':'zh');}catch{}
document.querySelectorAll('[data-layer]').forEach(button=>button.addEventListener('click',()=>{
 const diagram=document.querySelector('#topology');
 diagram.dataset.view=button.dataset.layer;
 document.querySelectorAll('[data-layer]').forEach(item=>item.setAttribute('aria-pressed',String(item===button)));
}));
