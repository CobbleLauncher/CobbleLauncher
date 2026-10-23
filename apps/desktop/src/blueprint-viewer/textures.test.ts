// SPDX-License-Identifier: MIT
import {expect,it} from 'vitest';
import {textureLayout,textureFragment,type ViewerTextures} from './textures';
it('maps six native face directions to deterministic atlas slots and leaves unknowns colored',()=>{
 const value:ViewerTextures={source:'game',textures:[{id:'z',dataUrl:''},{id:'a',dataUrl:''}],paletteFaces:[['z','a',null,null,'z',null]],resolvedBlocks:1};
 const result=textureLayout(value);expect(result.ids).toEqual(['a','z']);expect(result.side).toBe(2);expect(Array.from(result.mapping.filter((_,index)=>index%4===0))).toEqual([2,1,0,0,2,0]);
});
it('encodes all256 texture slots without aliasing missing faces and rejects GPU budgets',()=>{const textures=Array.from({length:256},(_,index)=>({id:String(index).padStart(3,'0'),dataUrl:''}));const result=textureLayout({source:'game',textures,paletteFaces:[['255',null,null,null,null,null]],resolvedBlocks:1});expect(result.mapping[0]).toBe(0);expect(result.mapping[1]).toBe(1);expect(()=>textureLayout({source:'none',textures,paletteFaces:[],resolvedBlocks:0},512)).toThrow('TEXTURE_LIMIT');expect(textureFragment).toContain('diffuseColor.rgb=sampled.rgb');expect(textureFragment).toContain('discard');});
