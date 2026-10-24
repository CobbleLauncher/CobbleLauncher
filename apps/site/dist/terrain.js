// SPDX-License-Identifier: MIT
// Original procedural illustration. No Minecraft textures or game assets.
const palettes = {
  meadow: { grass:['#a8bc76','#a2b874','#99ae69','#b1c381'], soil:'#9a8e70', rock:'#a3ac9c', leaf:'#6f9053', water:'#8fb9b0', sand:'#cfc9a4' },
  autumn: { grass:['#b9b57d','#c1b684','#b2af76','#c5be89'], soil:'#99856a', rock:'#a7aaa0', leaf:'#b47d45', water:'#90a9a6', sand:'#d2c7a7' },
  snow: { grass:['#e0e8df','#d5dfd8','#e9eee6','#cedbd6'], soil:'#8a9590', rock:'#b5bdb9', leaf:'#9bb4a5', water:'#98b7bf', sand:'#cedad4' },
};
const noise = (x,z) => { const n=Math.sin(x*127.1+z*311.7)*43758.5453; return n-Math.floor(n); };
const tint = (hex,factor) => '#'+hex.slice(1).match(/../g).map(v=>Math.max(0,Math.min(255,Math.round(parseInt(v,16)*factor))).toString(16).padStart(2,'0')).join('');

export function createTerrain(canvas) {
  const ctx=canvas.getContext('2d');
  let biome='meadow', turn=0, targetTurn=0, rotationFrame=0, queued=false, active=true;
  const terrain=[];
  for(let x=-12;x<=12;x++) for(let z=-11;z<=11;z++) {
    const distance=(x/12)**2+(z/10.5)**2;
    if(distance>1+noise(x,z)*.13) continue;
    const river=Math.abs(z-Math.sin(x*.22)*2)<1.3;
    const hill=Math.exp(-((x-2)**2+(z+5)**2)/38)*3;
    const height=river?3:4+Math.floor(hill+noise(x,z)*.6);
    terrain.push({x,z,height,river,sand:!river&&Math.abs(z-Math.sin(x*.22)*2)<2.4});
  }
  const surface=(x,z)=>terrain.find(c=>c.x===x&&c.z===z)?.height??4;
  function draw() {
    queued=false;
    if(!active) return;
    const rect=canvas.getBoundingClientRect();
    if(!rect.width) return;
    const dpr=Math.min(devicePixelRatio||1,2);
    canvas.width=Math.round(rect.width*dpr);canvas.height=Math.round(rect.height*dpr);
    const scale=Math.min(rect.width/810,rect.height/690)*.82;
    ctx.setTransform(dpr*scale,0,0,dpr*scale,rect.width*dpr/2,rect.height*dpr*.5);
    const p=palettes[biome], angle=turn*Math.PI/2;
    const rot=(x,z)=>[x*Math.cos(angle)-z*Math.sin(angle),x*Math.sin(angle)+z*Math.cos(angle)];
    const project=(x,y,z)=>{const [rx,rz]=rot(x,z);return [(rx-rz)*22,(rx+rz)*11-y*22];};
    const polygon=(points,color)=>{ctx.beginPath();points.forEach((v,i)=>i?ctx.lineTo(...v):ctx.moveTo(...v));ctx.closePath();ctx.fillStyle=color;ctx.fill();};
    function box(x,y,z,w,h,d,color,sideColor=color) {
      const corners=[[x,y,z],[x+w,y,z],[x+w,y,z+d],[x,y,z+d]];
      const upper=corners.map(([a,b,c])=>project(a,b+h,c));
      const lower=corners.map(v=>project(...v));
      const faces=[];
      for(let i=0;i<4;i++) {
        const j=(i+1)%4;
        const mid=rot((corners[i][0]+corners[j][0])/2-x-w/2,(corners[i][2]+corners[j][2])/2-z-d/2);
        if(mid[0]+mid[1]>0.001) faces.push({points:[lower[i],lower[j],upper[j],upper[i]],light:mid[0]>mid[1]?.78:.9});
      }
      faces.forEach(f=>polygon(f.points,tint(sideColor,f.light)));
      polygon(upper,color);
    }
    // A quiet drafting ellipse under the terrain, rather than an animated glow.
    ctx.save();ctx.scale(1,.38);ctx.beginPath();ctx.ellipse(0,300,300,215,0,0,Math.PI*2);ctx.fillStyle=biome==='snow'?'#84958b12':'#64734312';ctx.fill();ctx.restore();
    const sorted=[...terrain].sort((a,b)=>{const aa=rot(a.x,a.z),bb=rot(b.x,b.z);return aa[0]+aa[1]-bb[0]-bb[1];});
    sorted.forEach(c=>{
      const color=c.river?p.water:c.sand?p.sand:p.grass[Math.floor(noise(c.x,c.z)*4)];
      const bottom=1+Math.floor(noise(c.x+5,c.z)*1.7);
      box(c.x,-bottom,c.z,1,c.height+bottom-.2,1,tint(p.soil,.9+noise(c.x,c.z)*.12));
      box(c.x,c.height-.2,c.z,1,.23,1,color,c.river?p.water:p.soil);
      if(c.river&&noise(c.x+8,c.z)>.77) {
        const a=project(c.x+.2,c.height+.05,c.z+.4),b=project(c.x+.75,c.height+.05,c.z+.4);
        ctx.beginPath();ctx.moveTo(...a);ctx.lineTo(...b);ctx.strokeStyle='#e3ece0';ctx.lineWidth=.75;ctx.stroke();
      }
    });
    const details=[];
    function detail(x,z,render) {const r=rot(x,z);details.push({depth:r[0]+r[1],render});}
    // Stone arch, a small bridge, trees and wildflowers: all original cuboids.
    for(let x=-3;x<=3;x++) detail(x,1,()=>box(x,4,1,1,.35,3,'#9b977a'));
    const archTop=Math.max(surface(-7,-4),surface(-3,-4))+4;
    for(const x of [-7,-3]) for(let y=surface(x,-4);y<archTop;y++) detail(x,-4,()=>box(x,y,-4,1,1,1,p.rock));
    for(let x=-7;x<=-3;x++) detail(x,-4,()=>box(x,archTop,-4,1,1,1,p.rock));
    [[6,-4],[-7,4],[4,5],[1,-7],[8,1],[-2,7]].forEach(([x,z],index)=>detail(x,z,()=>{
      const y=surface(x,z), height=index%2?4:5;
      box(x+.3,y,z+.3,.4,height,.4,'#82745b');
      box(x-.6,y+height-1.3,z-.6,2.2,1.6,2.2,tint(p.leaf,.98+index*.025));
      box(x-.2,y+height+.3,z-.2,1.4,.8,1.4,tint(p.leaf,1.12));
      if(biome==='snow')box(x-.2,y+height+1.1,z-.2,1.4,.15,1.4,'#e7eee4');
    }));
    [[-9,-1],[-8,3],[7,4],[4,-8],[0,8],[9,-2]].forEach(([x,z],i)=>detail(x,z,()=>box(x+.25,surface(x,z),z+.25,.45,.3,.45,i%2?p.rock:'#d5cc8d')));
    details.sort((a,b)=>a.depth-b.depth).forEach(d=>d.render());
  }
  function schedule() { if(!queued&&active){queued=true;requestAnimationFrame(draw);} }
  const resize=new ResizeObserver(schedule);resize.observe(canvas);
  const visibility=new IntersectionObserver(([entry])=>{active=entry.isIntersecting;if(active)schedule();});visibility.observe(canvas);
  document.addEventListener('visibilitychange',()=>{active=!document.hidden;if(active)schedule();});
  schedule();
  return {
    setBiome(value){if(palettes[value]){biome=value;schedule();}},
    rotate(){
      targetTurn++;
      cancelAnimationFrame(rotationFrame);
      if(matchMedia('(prefers-reduced-motion: reduce)').matches){turn=targetTurn;schedule();return targetTurn%4;}
      const from=turn,start=performance.now();
      function step(now){
        const progress=Math.min(1,(now-start)/440);
        turn=from+(targetTurn-from)*(1-(1-progress)**3);
        draw();
        if(progress<1&&active)rotationFrame=requestAnimationFrame(step);
        else if(!active)turn=targetTurn;
      }
      rotationFrame=requestAnimationFrame(step);
      return targetTurn%4;
    },
    redraw:schedule,
  };
}
