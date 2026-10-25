// SPDX-License-Identifier: MIT
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createClient,MATERIALS_IMPORT_INTERFACE as interfaceId,MATERIALS_IMPORT_VERSION as version} from './index.ts';
import {validateMaterialsPayload} from './materials.ts';
test('full thousand material requirements use an unscoped typed handoff',async()=>{
 const requests:unknown[]=[];const client=createClient(async r=>{requests.push(r);return {requested:true}});
 const materials=Array.from({length:1000},(_,i)=>({id:`fixture:block_${i}`,required:i+1,stackSize:null}));
 await client.handoffs.send({interfaceId,version,targetPluginId:'material-planner',payload:{name:'Fixture',materials}});
 const r=requests[0] as {params:{gameId?:string;payload:{materials:unknown[]}}};assert.equal(r.params.gameId,undefined);assert.equal(r.params.payload.materials.length,1000);
 assert.throws(()=>client.handoffs.send({interfaceId,version,targetPluginId:'material-planner',worldId:'world',payload:{name:'Fixture',materials}}),/CONTEXT/);
});
test('requirements are bounded without truncation or guessed stacks',()=>{
 assert.equal(validateMaterialsPayload({name:'Fixture',materials:[{id:'minecraft:stone',required:100}]}).materials[0].stackSize,null);
 for(const materials of [[{id:'x',required:0}],[{id:'x',required:1,stackSize:1000}],[{id:'x',required:1},{id:'x',required:2}],Array.from({length:2049},(_,i)=>({id:`x${i}`,required:1}))])assert.throws(()=>validateMaterialsPayload({name:'Fixture',materials}));
});
test('native text export forwards full text and has bounded suggested names',async()=>{
 const requests:unknown[]=[];const c=createClient(async r=>{requests.push(r);return {saved:false}});
 assert.deepEqual(await c.exports.text({name:'redacted-log',text:'a'.repeat(1_048_576)}),{saved:false});
 assert.equal(requests.length,1);assert.throws(()=>c.exports.text({name:'../x',text:'a'}));assert.throws(()=>c.exports.text({name:'log',text:'a'.repeat(1_048_577)}));
});
