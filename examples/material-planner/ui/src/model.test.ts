// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { importedPlan, groups, missing, sortedItems, validatePlan, exportName } from './model.ts';
test('complete 1000-material import preserves requirements and explicit unknown stacks', () => {
    const plan = importedPlan({name:'Build',materials:Array.from({length:1000},(_,i)=>({id:`minecraft:block_${i}`,required:i+1,stackSize:null}))});
    assert.equal(plan.items.length,1000); assert.equal(plan.items[999]!.required,1000);
    assert.equal(plan.items[0]!.stackSize,null); assert.equal(plan.items[0]!.collected,0);
    plan.items[0]!.collected=99; assert.equal(missing(plan.items[0]!),0);
    assert.deepEqual(groups(129,64),{stacks:2,loose:1}); assert.equal(groups(129,null),null);
    assert.equal(sortedItems(plan,'block_999','missing').length,1);
});
test('long blockstate and UTF8 boundary accepted without coercion',()=>{
    const id='x'.repeat(1024), plan=importedPlan({name:'Long',materials:[{id,required:1}]});
    assert.equal(plan.items[0]!.label,id);
    assert.throws(()=>importedPlan({name:'Long',materials:[{id:'界'.repeat(342),required:1}]}),/PLAN_INVALID/);
    assert.throws(()=>validatePlan({...plan,id:'a'.repeat(24),items:[{...plan.items[0],required:1.5}]}));
    assert.equal(exportName({...plan,name:' ..name.. '}),'name.txt');
});
