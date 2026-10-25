// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { createClient } from './index.ts';
test('generated blueprints carry compact occupancy and expose only opaque open IDs',async()=>{
 const requests:unknown[]=[];const client=createClient(async request=>{requests.push(request);return {blueprintId:'one',name:'Cube'};});
 await client.generatedBlueprint.save({name:'Cube',width:2,height:2,length:2,block:'minecraft:stone',occupancy:'gQ=='});
 await client.generatedBlueprint.open('one');
 assert.deepEqual(requests,[{method:'generatedBlueprint.save',params:{name:'Cube',width:2,height:2,length:2,block:'minecraft:stone',occupancy:'gQ=='}},{method:'generatedBlueprint.open',params:{blueprintId:'one'}}]);
 assert.throws(()=>client.generatedBlueprint.open('../world'));
 assert.throws(()=>client.generatedBlueprint.save({name:'Cube',width:128,height:128,length:128,block:'minecraft:stone',occupancy:''}));
});
