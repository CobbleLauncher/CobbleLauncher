// SPDX-License-Identifier: MIT
import {writeFile} from 'node:fs/promises';
import {legal} from './page-content.mjs';
import {meshContent} from './mesh-content.mjs';
const route=(page,lang)=>`./${page}${lang==='en'?'.en':''}.html`;
function shell(page,lang,title,description,body){
 const en=lang==='en',t=(zh,enText)=>en?enText:zh;
 return `<!doctype html>
<html lang="${en?'en':'zh-CN'}">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark">
<title>${title} — Cobble</title><meta name="description" content="${description}"><link rel="icon" href="./favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="./style.css"><link rel="stylesheet" href="./pages.css"><script type="module" src="./page.js"></script>
<link rel="alternate" hreflang="zh-CN" href="${route(page,'zh')}"><link rel="alternate" hreflang="en" href="${route(page,'en')}"></head>
<body><a class="skip-link" href="#main">${t('跳到内容','Skip to content')}</a>
<header class="page-header"><a class="brand" href="./"><img src="./favicon.svg" width="28" height="28" alt="">cobble</a><nav class="page-nav" aria-label="${t('主导航','Main navigation')}">${[['mesh','Mesh'],['privacy',t('隐私','Privacy')],['terms',t('条款','Terms')]].map(([id,label])=>`<a href="${route(id,lang)}" ${page===id?'class="current" aria-current="page"':''}>${label}</a>`).join('')}</nav><div class="page-actions"><button class="icon-button" data-theme-toggle aria-label="${t('切换浅色或深色主题','Toggle light or dark theme')}" aria-pressed="false"><svg class="icon" aria-hidden="true"><use href="./icons.svg#sun-moon"/></svg></button><a class="language" href="${route(page,en?'zh':'en')}" data-language-link="${en?'zh':'en'}" aria-label="${en?'切换为中文':'Switch to English'}">${en?'中':'EN'}</a></div></header>
<main id="main" class="page-main">${body}</main>
<footer class="page-footer"><a href="./">© 2026 Cobble</a><nav aria-label="${t('页脚导航','Footer navigation')}"><a href="${route('mesh',lang)}">Mesh</a><a href="${route('privacy',lang)}">${t('隐私政策','Privacy')}</a><a href="${route('terms',lang)}">${t('使用条款','Terms')}</a><a href="https://github.com/CobbleLauncher/CobbleLauncher">GitHub ↗</a></nav></footer></body></html>
`;
}
for(const lang of ['zh','en']){
 for(const page of ['privacy','terms']){
  const content=legal[page][lang];
  const body=`<div class="page-lead"><h1>${content.title}</h1><p>${content.intro}</p><p class="page-meta">${lang==='en'?'Updated':'更新日期'} · 2026-10-25</p></div><div class="legal-layout"><nav class="page-toc" aria-label="${lang==='en'?'On this page':'本页目录'}">${content.sections.map(([name],i)=>`<a href="#section-${i+1}">${name}</a>`).join('')}</nav><div class="legal-body">${content.sections.map(([name,...paragraphs],i)=>`<section id="section-${i+1}"><h2>${name}</h2>${paragraphs.map(p=>`<p>${p}</p>`).join('')}</section>`).join('')}</div></div>`;
  await writeFile(new URL(route(page,lang).replace('./','./dist/'),import.meta.url),shell(page,lang,content.title,content.intro,body));
 }
 await writeFile(new URL(route('mesh',lang).replace('./','./dist/'),import.meta.url),shell('mesh',lang,'Cobble Mesh',lang==='en'?'Local identities, discovery, relays and game streams. Explore the architecture of Cobble Mesh.':'本地身份、节点发现、中继与游戏流：用拓扑和时序图了解 Cobble Mesh。',meshContent(lang)));
}
