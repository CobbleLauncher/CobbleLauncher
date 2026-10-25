// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import type { Client } from '../../../../packages/plugin-web-sdk/src/index.ts';
import { importedPlan } from './model.ts';
import { encodedBytes, PlanRepository, splitPlan } from './storage.ts';
function host() {
    const docs=new Map<string,{value:unknown;revision:string}>(); let serial=0, failAt=0, writes=0;
    const key=(scope:unknown,key:string)=>JSON.stringify(scope)+key;
    const data={
        async read(args:{scope:unknown;key:string}) {return structuredClone(docs.get(key(args.scope,args.key))??{value:null,revision:null});},
        async write(args:{scope:unknown;key:string;value:unknown;expectedRevision:string|null}) {
            writes++; if(writes===failAt)throw Error('INJECTED');
            const id=key(args.scope,args.key), previous=docs.get(id);
            if((previous?.revision??null)!==args.expectedRevision)throw Error('PLUGIN_DATA_CONFLICT');
            assert.ok(encodedBytes(args.value)<=65536);
            const document={value:structuredClone(args.value),revision:`r${++serial}`};docs.set(id,document);return structuredClone(document);
        },
        async remove(args:{scope:unknown;key:string;expectedRevision:string|null}) {
            const id=key(args.scope,args.key);
            if((docs.get(id)?.revision??null)!==args.expectedRevision)throw Error('PLUGIN_DATA_CONFLICT');
            docs.delete(id);return null;
        }
    } as Client['data'];
    return {data,docs,failNext(offset=1){failAt=writes+offset;}};
}
let generation=1;
const next=()=>(generation++).toString(16).padEnd(24,'0');
function fixture(count=1000,long=false) {
    const plan=importedPlan({name:'Build',materials:Array.from({length:count},(_,i)=>({id:`minecraft:block_${i}${long?'x'.repeat(950):''}`,required:i+1}))});
    plan.id=next();return plan;
}
test('1000 material progress saves/reopens in global scope, chunks <=60KiB',async()=>{
    const h=host(),repo=new PlanRepository(h.data,next),plan=fixture();
    plan.items[999]!.collected=777;plan.items[0]!.stackSize=16;
    await repo.loadIndex({});await repo.save(plan);
    const reopened=new PlanRepository(h.data,next);await reopened.loadIndex({});
    assert.deepEqual(await reopened.loadPlan(plan.id),plan);
    for(const chunk of splitPlan(plan,next()))assert.ok(encodedBytes(chunk)<=60*1024);
    await reopened.loadIndex({gameId:'other'});assert.equal(reopened.index.plans.length,0);
});
test('staged failure retains prior saved version and new-plan retry works',async()=>{
    const h=host(),repo=new PlanRepository(h.data,next),plan=fixture();await repo.loadIndex({});
    h.failNext(3);await assert.rejects(repo.save(plan),/INJECTED/);
    await repo.save(plan);const before=structuredClone(plan);plan.items[500]!.collected=99;
    h.failNext(2);await assert.rejects(repo.save(plan),/INJECTED/);
    const other=new PlanRepository(h.data,next);await other.loadIndex({});assert.deepEqual(await other.loadPlan(plan.id),before);
    await repo.save(plan);assert.deepEqual(await other.loadPlan(plan.id),plan);
});
test('repeated save garbage remains bounded and conflicts preserve draft',async()=>{
    const h=host(),repo=new PlanRepository(h.data,next),plan=fixture();await repo.loadIndex({});await repo.save(plan);
    for(let i=0;i<12;i++){plan.items[0]!.collected=i;await repo.save(plan);}
    const manifests=[...h.docs.values()].map(doc=>doc.value).filter((v:any)=>v?.chunks) as {garbage:unknown[]}[];
    assert.ok(manifests[0]!.garbage.length<=3);
    const other=new PlanRepository(h.data,next);await other.loadIndex({});const stale=await other.loadPlan(plan.id);
    plan.items[1]!.collected=2;await repo.save(plan);stale.items[1]!.collected=55;
    await assert.rejects(other.save(stale),/CONFLICT/);assert.equal(stale.items[1]!.collected,55);
});
test('more than sixteen chunks reopen without flooding bridge',async()=>{
    const h=host(),repo=new PlanRepository(h.data,next),plan=fixture(1000,true);
    assert.ok(splitPlan(plan,next()).length>16);
    assert.ok(encodedBytes({name:plan.name,materials:plan.items.map(item=>({id:item.id,required:item.required,stackSize:null}))})<=1048576);
    await repo.loadIndex({});await repo.save(plan);
    const reopened=new PlanRepository(h.data,next);await reopened.loadIndex({});assert.deepEqual(await reopened.loadPlan(plan.id),plan);
});

