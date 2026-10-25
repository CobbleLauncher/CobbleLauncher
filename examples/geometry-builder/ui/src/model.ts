// SPDX-License-Identifier: MIT
export type Shape = 'circle'|'ellipse'|'sphere'|'dome';
export const materials = ['minecraft:stone','minecraft:cobblestone','minecraft:glass','minecraft:oak_planks','minecraft:white_concrete'] as const;
export type Request = {shape:Shape;width:number;depth:number;hollow:boolean;material:string};
export type Block = {x:number;y:number;z:number;paletteIndex:0};
export type Result = {schemaVersion:1;name:string;dimensions:{x:number;y:number;z:number};palette:(typeof materials[number])[];blocks:Block[];layers:Block[][]};
export function occupancy(result:Result):string {
 const {x:w,y:h,z:d}=result.dimensions,bytes=new Uint8Array(Math.ceil(w*h*d/8));
 for(const b of result.blocks){const i=b.x+b.z*w+b.y*w*d;bytes[i>>3]!|=1<<(i&7);}
 let binary='';for(const byte of bytes)binary+=String.fromCharCode(byte);return btoa(binary);
}
export function validate(r:Request) {
 if(!['circle','ellipse','sphere','dome'].includes(r.shape)||!Number.isInteger(r.width)||r.width<1||r.width>128||!Number.isInteger(r.depth)||r.depth<1||r.depth>128||!materials.some(m=>m===r.material)||typeof r.hollow!=='boolean')throw Error('INVALID_SHAPE');
}
// Cell centres inside the analytical ellipsoid; the shell is its six-neighbour boundary.
// For flat shapes the boundary uses four neighbours, so hollow circles are one cell thick.
export async function generate(r:Request,signal?:AbortSignal):Promise<Result> {
 validate(r);const flat=r.shape==='circle'||r.shape==='ellipse',w=r.width,d=r.shape==='ellipse'?r.depth:w;
 const fullHeight=flat?1:w,firstY=r.shape==='dome'?Math.floor(w/2):0,h=fullHeight-firstY;
 if(w*h*d>1048576)throw Error('VOLUME_LIMIT');
 const inside=(x:number,y:number,z:number)=>{
  if(x<0||x>=w||z<0||z>=d||y<0||y>=fullHeight)return false;
  const nx=(x-(w-1)/2)/(w/2),nz=(z-(d-1)/2)/(d/2),ny=flat?0:(y-(w-1)/2)/(w/2);
  return nx*nx+ny*ny+nz*nz<=1+1e-12;
 };
 const layers:Block[][]=[],blocks:Block[]=[];let visited=0;
 for(let y=firstY;y<fullHeight;y++){
  const layer:Block[]=[];
  for(let z=0;z<d;z++)for(let x=0;x<w;x++){
   if(++visited%4096===0){await new Promise<void>(resolve=>setTimeout(resolve,0));if(signal?.aborted)throw Error('CANCELLED');}
   if(signal?.aborted)throw Error('CANCELLED');
   if(!inside(x,y,z))continue;
   const boundary=!inside(x-1,y,z)||!inside(x+1,y,z)||!inside(x,y,z-1)||!inside(x,y,z+1)||(!flat&&(!inside(x,y-1,z)||!inside(x,y+1,z)));
   if(r.hollow&&!boundary)continue;
   if(blocks.length>=262144)throw Error('BLOCK_LIMIT');
   const block:Block={x,y:y-firstY,z,paletteIndex:0};layer.push(block);blocks.push(block);
  }
  layers.push(layer);
 }
 return {schemaVersion:1,name:`${r.shape} ${w}×${h}×${d}`,dimensions:{x:w,y:h,z:d},palette:[materials.find(m=>m===r.material)!],blocks,layers};
}
