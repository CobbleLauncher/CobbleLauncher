// SPDX-License-Identifier: MIT
import type { Transport } from './index.ts';
export type GeneratedBlueprint = {name:string;width:number;height:number;length:number;block:'minecraft:stone'|'minecraft:cobblestone'|'minecraft:glass'|'minecraft:oak_planks'|'minecraft:white_concrete';occupancy:string};
export type SavedGeneratedBlueprint = {blueprintId:string;name:string};
export function createGeneratedBlueprint(transport:Transport){return Object.freeze({
 save:(args:GeneratedBlueprint)=>{
  const volume=args.width*args.height*args.length;
  if(volume>1048576||![args.width,args.height,args.length].every(n=>Number.isInteger(n)&&n>=1&&n<=128)||!args.name.trim()||[...args.name].length>80||/[\u0000-\u001f\u007f]/u.test(args.name)||!['minecraft:stone','minecraft:cobblestone','minecraft:glass','minecraft:oak_planks','minecraft:white_concrete'].includes(args.block)||typeof args.occupancy!=='string'||args.occupancy.length!==4*Math.ceil(Math.ceil(volume/8)/3))throw new Error('GENERATED_BLUEPRINT_INVALID');
  return transport({method:'generatedBlueprint.save',params:args}) as Promise<SavedGeneratedBlueprint>;
 },
 open:(blueprintId:string)=>{if(!/^[a-zA-Z0-9_-]{1,128}$/.test(blueprintId))throw new Error('GENERATED_BLUEPRINT_INVALID');return transport({method:'generatedBlueprint.open',params:{blueprintId}}) as Promise<{requested:true}>;}
});}
