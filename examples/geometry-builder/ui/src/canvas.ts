// SPDX-License-Identifier: MIT
import type {Result} from './model.ts';
export function drawLayer(canvas:HTMLCanvasElement,result:Result,index:number){
 const ctx=canvas.getContext('2d');if(!ctx)return;
 const {x:w,z:d}=result.dimensions,size=640,pad=32,cell=(size-pad*2)/Math.max(w,d),ox=(size-w*cell)/2,oz=(size-d*cell)/2;
 canvas.width=size;canvas.height=size;ctx.fillStyle='#15231d';ctx.fillRect(0,0,size,size);
 ctx.fillStyle='#8acaa5';for(const b of result.layers[index]??[])ctx.fillRect(ox+b.x*cell+.4,oz+b.z*cell+.4,Math.max(.8,cell-.8),Math.max(.8,cell-.8));
 if(cell>=7){ctx.strokeStyle='#ffffff20';ctx.lineWidth=.5;ctx.beginPath();for(let x=0;x<=w;x++){ctx.moveTo(ox+x*cell,oz);ctx.lineTo(ox+x*cell,oz+d*cell);}for(let z=0;z<=d;z++){ctx.moveTo(ox,oz+z*cell);ctx.lineTo(ox+w*cell,oz+z*cell);}ctx.stroke();}
 ctx.strokeStyle='#ffcf73';ctx.lineWidth=1.5;ctx.setLineDash([5,4]);ctx.beginPath();ctx.moveTo(size/2,oz-10);ctx.lineTo(size/2,oz+d*cell+10);ctx.moveTo(ox-10,size/2);ctx.lineTo(ox+w*cell+10,size/2);ctx.stroke();ctx.setLineDash([]);
 ctx.fillStyle='#edf5ee';ctx.font='14px sans-serif';ctx.fillText('X →',size-44,22);ctx.fillText('Z ↓',8,size-10);
}
