// SPDX-License-Identifier: MIT
export type ViewerTextures = {source:'game'|'none';textures:{id:string;dataUrl:string}[];paletteFaces:(string|null)[][];resolvedBlocks:number};
export const TILE_SIZE=64;
/** Stable slots make atlas and palette-face lookup independent of decode order. */
export function textureLayout(value:ViewerTextures,maxSize=4096){
 const ids=[...new Set(value.textures.map(texture=>texture.id))].sort();
 if(ids.length>256||value.paletteFaces.length>1024)throw Error('TEXTURE_LIMIT');
 const side=Math.max(1,Math.ceil(Math.sqrt(ids.length)));
 if(side*TILE_SIZE>maxSize||Math.max(6,value.paletteFaces.length)>maxSize)throw Error('TEXTURE_LIMIT');
 const slots=new Map(ids.map((id,index)=>[id,index]));
 const mapping=new Uint8Array(Math.max(1,value.paletteFaces.length)*6*4);
 for(const [palette,faces] of value.paletteFaces.entries())for(let face=0;face<6;face++){
  const id=faces[face];const slot=id?slots.get(id):undefined;if(slot===undefined)continue;
  const code=slot+1,offset=(palette*6+face)*4;mapping[offset]=code&255;mapping[offset+1]=code>>>8;mapping[offset+3]=255;
 }
 return{ids,side,size:side*TILE_SIZE,mapping};
}
export async function textureAtlas(value:ViewerTextures,signal:AbortSignal,maxSize:number){
 const layout=textureLayout(value,maxSize);const canvas=document.createElement('canvas');canvas.width=canvas.height=layout.size;
 const context=canvas.getContext('2d');if(!context)throw Error('TEXTURE_CANVAS_UNAVAILABLE');context.imageSmoothingEnabled=false;
 const byId=new Map(value.textures.map(texture=>[texture.id,texture.dataUrl]));
 await Promise.all(layout.ids.map(async(id,index)=>{
  const source=byId.get(id);if(!source?.startsWith('data:image/png;base64,')||source.length>1048576)throw Error('INVALID_TEXTURE');
  const image=new Image();image.src=source;await image.decode();if(signal.aborted)return;
  if(image.width<1||image.height<image.width||image.width>64)throw Error('INVALID_TEXTURE');
  context.drawImage(image,0,0,image.width,image.width,index%layout.side*TILE_SIZE,Math.floor(index/layout.side)*TILE_SIZE,TILE_SIZE,TILE_SIZE);
 }));
 return{...layout,canvas};
}
/** Pinned Three Lambert shader hooks; sampled diffuse is still lit by Lambert. */
export const textureVertexHeader=`attribute float paletteIndex;
attribute float faceId;
varying vec2 blueprintUv;
varying float blueprintPalette;
varying float blueprintFace;
`;
export const textureVertex=`#include <uv_vertex>
blueprintUv=uv; blueprintPalette=paletteIndex; blueprintFace=faceId;`;
export const textureFragmentHeader=`uniform sampler2D blueprintAtlas;
uniform sampler2D blueprintFaces;
uniform float blueprintPaletteCount;
uniform float blueprintAtlasSide;
uniform float blueprintTexturesEnabled;
varying vec2 blueprintUv;
varying float blueprintPalette;
varying float blueprintFace;
`;
export const textureFragment=`#include <color_fragment>
if(blueprintTexturesEnabled>0.5&&blueprintPalette>=0.0&&blueprintPalette<blueprintPaletteCount){
 vec4 entry=texture2D(blueprintFaces,vec2((blueprintFace+0.5)/6.0,(blueprintPalette+0.5)/blueprintPaletteCount));
 float code=floor(entry.r*255.0+0.5)+floor(entry.g*255.0+0.5)*256.0;
 if(code>0.5){
  float slot=code-1.0;
  vec2 tile=vec2(mod(slot,blueprintAtlasSide),floor(slot/blueprintAtlasSide));
  vec2 atlasUv=vec2((tile.x+blueprintUv.x)/blueprintAtlasSide,1.0-(tile.y+1.0-blueprintUv.y)/blueprintAtlasSide);
  vec4 sampled=texture2D(blueprintAtlas,atlasUv);
  if(sampled.a<0.1)discard;
  diffuseColor.rgb=sampled.rgb; diffuseColor.a*=sampled.a;
 }
}`;
