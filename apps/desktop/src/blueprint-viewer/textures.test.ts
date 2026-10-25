// SPDX-License-Identifier: MIT
import {expect,it,vi} from 'vitest';
import {textureAtlas,textureLayout,textureFragment,type ViewerTextures} from './textures';
it('maps six native face directions to deterministic atlas slots and leaves unknowns colored',()=>{
 const value:ViewerTextures={source:'game',textures:[{id:'z',dataUrl:''},{id:'a',dataUrl:''}],paletteFaces:[['z','a',null,null,'z',null]],resolvedBlocks:1};
 const result=textureLayout(value);expect(result.ids).toEqual(['a','z']);expect(result.side).toBe(2);expect(Array.from(result.mapping.filter((_,index)=>index%4===0))).toEqual([2,1,0,0,2,0]);
});
it('encodes all256 texture slots without aliasing missing faces and rejects GPU budgets',()=>{const textures=Array.from({length:256},(_,index)=>({id:String(index).padStart(3,'0'),dataUrl:''}));const result=textureLayout({source:'game',textures,paletteFaces:[['255',null,null,null,null,null]],resolvedBlocks:1});expect(result.mapping[0]).toBe(0);expect(result.mapping[1]).toBe(1);expect(()=>textureLayout({source:'none',textures,paletteFaces:[],resolvedBlocks:0},512)).toThrow('TEXTURE_LIMIT');expect(textureFragment).toContain('diffuseColor.rgb=sampled.rgb');expect(textureFragment).toContain('discard');});
it('cancels pending image decodes and drops decoded source references when the viewer closes',async()=>{
 const source='data:image/png;base64,AA==';let finish!:()=>void;let created!:HTMLImageElement;
 vi.stubGlobal('Image',class {src='';width=16;height=16;decode(){return new Promise<void>(resolve=>{finish=resolve;});}constructor(){created=this as unknown as HTMLImageElement;}});
 const createElement=document.createElement.bind(document);vi.spyOn(document,'createElement').mockImplementation(((tag:string)=>tag==='canvas'?{width:0,height:0,getContext:()=>({imageSmoothingEnabled:true,drawImage:vi.fn()})} as unknown as HTMLCanvasElement:createElement(tag)) as typeof document.createElement);
 const controller=new AbortController();const pending=textureAtlas({source:'game',textures:[{id:'stone',dataUrl:source}],paletteFaces:[],resolvedBlocks:1},controller.signal,4096);
 await Promise.resolve();controller.abort();await expect(pending).rejects.toThrow('ABORTED');expect(created.src).toBe('');
 finish();vi.unstubAllGlobals();vi.restoreAllMocks();
});
