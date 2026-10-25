// SPDX-License-Identifier: MIT
import { demoCopy } from './copy.js';
export const icon=name=>`<svg class="icon" aria-hidden="true"><use href="./icons.svg#${name}"/></svg>`;
const landscape=`<svg class="cover-land" viewBox="0 0 600 210" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><rect width="600" height="210" fill="#dce5bf"/><circle cx="480" cy="42" r="24" fill="#f2efcc"/><path d="M0 100h50V85h45v20h45V65h40v40h45V90h35v30h50V70h55v30h40V55h45v25h55V50h50v40h45v120H0Z" fill="#aebe8d"/><path d="M0 150h55v-20h60v17h45v-10h50v-25h50v30h45v-28h55v13h45v-20h55v30h60v-25h60v98H0Z" fill="#8fa573"/><path d="M0 195v-24h90v10h90v-20h80v16h70v-20h60v20h80v-13h130v46H0Z" fill="#bed0a0"/><path d="M408 111h42v48h-42z" fill="#e8dfbc"/><path d="m397 111 33-29 34 29Z" fill="#5c7354"/><path d="M423 135h13v24h-13z" fill="#5c7354"/><path d="M361 110h8v55h-8z" fill="#667b56"/><path d="M344 102h41v26h-41zM350 82h29v24h-29z" fill="#6f875c"/></svg>`;

export function renderProduct(container, feature, lang, state) {
  const t=demoCopy[lang];
  if(feature==='play') {
    container.innerHTML=`<div class="demo-greeting"><strong>${t.welcome}</strong><span>${t.last}</span></div><div class="game-cover">${landscape}<div class="game-caption"><h3>${t.world}</h3><p>1.21.1 · Fabric</p></div><button class="demo-play" aria-pressed="${state.running}">${icon(state.running?'check':'play')}<span>${state.running?t.running:t.play}</span></button></div><div class="mini-games"><div class="mini-game"><span class="game-icon">${icon('zap')}</span><span>${t.creative}<small>1.20.1 · Fabric</small></span></div><div class="mini-game"><span class="game-icon">${icon('mountain')}</span><span>${t.adventure}<small>1.21.1 · Vanilla</small></span></div></div>`;
    container.querySelector('.demo-play').addEventListener('click',()=>{state.running=!state.running;renderProduct(container,feature,lang,state);container.querySelector('.demo-play').focus();document.querySelector('#announcement').textContent=t.playResult;});
  } else if(feature==='mods') {
    container.innerHTML=`<div class="demo-greeting"><strong>${t.mods}</strong><span>${t.matched}</span></div>${[['Sodium',t.performance,'layers'],['Iris',t.shader,'sun-moon'],['Fabric API',t.dependency,'blocks']].map(([title,desc,symbol])=>`<div class="mod-row"><span class="game-icon">${icon(symbol)}</span><div>${title}<small>${desc}</small></div><span class="mod-status">${t.compatible}</span></div>`).join('')}<p class="demo-footnote">${icon('check')}${t.dependencies}</p>`;
  } else {
    container.innerHTML=`<div class="demo-greeting"><strong>${t.worlds}</strong><span>${t.automatic}</span></div>${[[t.snapshot1,t.today,'+ 4.2 MB'],[t.snapshot2,t.yesterday,'+ 2.8 MB'],[t.snapshot3,t.daysAgo,'128 MB']].map(([name,date,size])=>`<div class="snapshot-row">${icon('history')}<div>${name}<small>${date}</small></div><span>${size}</span></div>`).join('')}<p class="demo-footnote">${icon('hard-drive')}${t.dedupe}</p>`;
  }
}

export function circleOutline(radius) {
  const cells=[];
  for(let y=-radius;y<=radius;y++)for(let x=-radius;x<=radius;x++) {
    if(Math.abs(Math.hypot(x,y)-radius)<.5)cells.push([x,y]);
  }
  return cells;
}
function drawCircle(canvas,radius) {
  const ctx=canvas.getContext('2d'),size=520,dpr=2;
  canvas.width=size;canvas.height=size;
  const pad=28,unit=(size-pad*2)/(radius*2+1);
  ctx.clearRect(0,0,size,size);
  const cells=circleOutline(radius);
  // A tiny drafting grid with a measured center mark.
  ctx.strokeStyle='#8ba57925';ctx.lineWidth=1;
  for(let i=0;i<=radius*2+1;i++){const at=pad+i*unit;ctx.beginPath();ctx.moveTo(at,pad);ctx.lineTo(at,size-pad);ctx.moveTo(pad,at);ctx.lineTo(size-pad,at);ctx.stroke();}
  cells.forEach(([x,y])=>{ctx.fillStyle=(x+y)%3===0?'#c5d699':'#9bb378';ctx.fillRect(pad+(x+radius)*unit+1,pad+(y+radius)*unit+1,unit-2,unit-2);});
  ctx.strokeStyle='#e3eac7';ctx.lineWidth=dpr/2;ctx.beginPath();ctx.moveTo(250,260);ctx.lineTo(270,260);ctx.moveTo(260,250);ctx.lineTo(260,270);ctx.stroke();
  return cells.length;
}
function mapSvg(location) {
  const pin=location==='village'?[115,100]:[232,170];
  return `<svg class="compass-map" viewBox="0 0 300 280" role="img" aria-label="${location==='village'?'Village / 村庄':'Desert temple / 沙漠神殿'}"><rect width="300" height="280" fill="#b4c18c"/><path d="M0 0h120v30h30v35h-35v30H80v30H40v50H0zM0 230h40v-35h30v45h40v40H0z" fill="#7d996a"/><path d="M210 0h90v280h-90v-25h-40v-55h30v-40h-25v-35h35z" fill="#d1c898"/><path d="M135 0h26v45h23v42h-20v44h-27v44h-28v40H85v65H65v-85h27v-40h29v-43h27V89h15V60h-28z" fill="#8eb0a1"/><path d="M0 70h300M0 140h300M0 210h300M75 0v280M150 0v280M225 0v280" fill="none" stroke="#384d3220" stroke-width="1"/><circle cx="${pin[0]}" cy="${pin[1]}" r="15" fill="#e6edc650"/><path d="m${pin[0]} ${pin[1]-9} 9 9-9 9-9-9Z" class="map-pin"/><text x="278" y="22" font-size="12" fill="#3d543c" font-family="monospace">N</text><path d="m282 29-4 10h8Z" fill="#3d543c"/></svg>`;
}
export function renderTool(container,tool,lang,state) {
  const t=demoCopy[lang];
  if(tool==='geometry') {
    container.innerHTML=`<div class="tool-visual"><span class="visual-label">${t.outline} / XY</span><canvas id="geometry-canvas" width="520" height="520" role="img" aria-label="${t.outline}"></canvas><span class="visual-measure" id="block-count"></span></div><div class="tool-copy"><span class="section-index">${t.geometryOver}</span><h3>${t.geometryTitle}</h3><p>${t.geometryDesc}</p><label class="range-control" for="radius"><span>${t.radius}</span><input id="radius" type="range" min="4" max="16" value="${state.radius}"><output for="radius">${state.radius}</output></label><span class="tag">${t.pluginTag}</span></div>`;
    const draw=()=>{const count=drawCircle(container.querySelector('canvas'),state.radius);container.querySelector('#block-count').textContent=`${count} ${t.blocks}`;container.querySelector('canvas').setAttribute('aria-label',`${t.radius} ${state.radius} / ${count} ${t.blocks}`);};
    container.querySelector('#radius').addEventListener('input',event=>{state.radius=Number(event.target.value);container.querySelector('output').value=state.radius;draw();});draw();
  } else if(tool==='compass') {
    container.innerHTML=`<div class="tool-visual"><span class="visual-label">${t.mapLabel}</span><div id="map-art">${mapSvg(state.location)}</div><span class="visual-measure">${state.location==='village'?'X 184 · Z −320':'X 672 · Z 144'}</span></div><div class="tool-copy"><span class="section-index">${t.compassOver}</span><h3>${t.compassTitle}</h3><p>${t.compassDesc}</p><div class="map-buttons" role="group" aria-label="${t.compassTitle}"><button data-place="village" aria-pressed="${state.location==='village'}">${t.village}</button><button data-place="temple" aria-pressed="${state.location==='temple'}">${t.temple}</button></div><span class="tag">${t.compassTag}</span></div>`;
    container.querySelectorAll('[data-place]').forEach(button=>button.addEventListener('click',()=>{state.location=button.dataset.place;renderTool(container,tool,lang,state);container.querySelector(`[data-place="${state.location}"]`).focus();}));
  } else {
    container.innerHTML=`<div class="tool-visual"><span class="visual-label">${t.sample} / CHANGE HISTORY</span><div class="diagnosis"><header>${icon('git-compare-arrows')} ${t.timeline}</header><ul><li><span>${t.memory}</span><span class="changed">4 GB → 2 GB</span></li><li><span>${t.modChange}</span><span class="changed">${t.changed}</span></li><li><span>Java</span><span class="unchanged">21 → 21</span></li><li><span>${t.settings}</span><span class="unchanged">${t.unchanged}</span></li></ul></div></div><div class="tool-copy"><span class="section-index">${t.crashOver}</span><h3>${t.crashTitle}</h3><p>${t.crashDesc}</p><span class="tag">${t.crashTag}</span></div>`;
  }
}
