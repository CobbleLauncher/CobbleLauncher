// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import {solve,solveAsync,steps,penalty,validate,type Book,type Request,type Node} from './model.ts';
import {byId,applicable} from './catalog.ts';
const book=(id:string,level:number,work=0):Book=>({id,level,work});
const request=(books:Book[],overrides:Partial<Request>={}):Request=>({version:'1.21.1',item:'sword',work:0,existing:[],books,creative:false,...overrides});
test('source-backed arithmetic: directional donor charge and all prior work',()=>{
 const plan=solve(request([book('mending',1,1)],{work:4,existing:[{id:'sharpness',level:5}]}));
 assert.equal(plan.root.total,15+1+2);assert.equal(plan.root.work,5);assert.equal(penalty(plan.root.work),31);
 assert.deepEqual(plan.root.enchants,[{id:'mending',level:1},{id:'sharpness',level:5}]);
 assert.deepEqual(steps(plan.root).map(s=>[s.left,s.right,s.cost]),[['item','book:1',18]]);
});
test('three-book sword optimum is 14 levels and preserves every input',()=>{
 const result=solve(request([book('sharpness',5),book('unbreaking',3),book('mending',1)]));
 assert.equal(result.root.total,14);assert.equal(result.survival,true);assert.equal(steps(result.root).length,3);
 const refs=steps(result.root).flatMap(s=>[s.left,s.right]);for(const ref of ['item','book:1','book:2','book:3'])assert.equal(refs.filter(r=>r===ref).length,1);
});
test('39 levels allowed, 40 refused per operation, Creative fallback disclosed',()=>{
 const allowed=solve(request([book('sharpness',5,2)],{work:5}));assert.equal(allowed.root.peak,39);assert.equal(allowed.survival,true);
 const refused=solve(request([book('looting',3,2)],{work:5}));assert.equal(refused.root.peak,40);assert.equal(refused.survival,false);
 const creative=solve(request([book('looting',3,2)],{work:5,creative:true}));assert.equal(creative.root.total,40);
});
test('duplicate books upgrade levels; equal-level merges must maximize the outcome before minimizing cost',()=>{
 const four=solve(request(Array.from({length:4},()=>book('sharpness',1))));assert.deepEqual(four.root.enchants,[{id:'sharpness',level:3}]);assert.equal(four.root.total,13);
 const existing=solve(request([book('sharpness',4)],{existing:[{id:'sharpness',level:4}]}));assert.deepEqual(existing.root.enchants,[{id:'sharpness',level:5}]);assert.equal(existing.root.total,5);
 const unequal=solve(request([book('sharpness',3)],{existing:[{id:'sharpness',level:4}]}));assert.equal(unequal.root.enchants[0]!.level,4);assert.equal(unequal.root.total,4);
});
test('incompatibilities, version/item/level boundaries and limits fail explicitly',()=>{
 assert.throws(()=>solve(request([book('sharpness',5),book('smite',5)])),/ENCHANT_CONFLICT/);
 assert.throws(()=>solve(request([book('mending',1),book('infinity',1)],{item:'bow'})),/ENCHANT_CONFLICT/);
 assert.throws(()=>solve(request([book('mending',1)],{version:'1.21.4'})),/VERSION_UNSUPPORTED/);
 assert.throws(()=>solve(request([book('mending',1)],{item:'mace'})),/ITEM_UNSUPPORTED/);
 assert.throws(()=>solve(request([book('sharpness',6)])),/LEVEL_INVALID/);
 assert.throws(()=>solve(request([book('sharpness',5)],{work:7})),/WORK_INVALID/);
 assert.throws(()=>solve(request([])),/BOOK_LIMIT/);assert.throws(()=>solve(request(Array.from({length:9},()=>book('mending',1)))),/BOOK_LIMIT/);
 assert.throws(()=>solve(request([book('mending',1)],{existing:[{id:'mending',level:1},{id:'mending',level:1}]})),/EXISTING_DUPLICATE/);
 assert.throws(()=>solve(request([book('mending',1),book('unbreaking',3)]),1),/SEARCH_LIMIT/);
 assert.doesNotThrow(()=>validate(request([book('fortune',3)],{item:'hoe',version:'1.20.1'})));
});
// Independent oracle: enumerate pair reductions rather than subset DP; distinct enchantments
// make the scalar donor value sufficient. No use of solve/merge for oracle arithmetic.
function oracle(input:Request):number{
 type State={gear:boolean;value:number;work:number};let best=Infinity;
 const values:State[]=[{gear:true,value:0,work:input.work},...input.books.map(e=>({gear:false,value:byId.get(e.id)!.weight*e.level,work:e.work}))];
 function visit(states:State[],total:number){if(total>=best)return;if(states.length===1){best=total;return;}
  for(let a=0;a<states.length;a++)for(let b=0;b<states.length;b++){if(a===b||states[b]!.gear)continue;const left=states[a]!,right=states[b]!,cost=2**left.work+2**right.work-2+right.value;if(cost>=40)continue;
   visit([...states.filter((_,i)=>i!==a&&i!==b),{gear:left.gear,value:left.value+right.value,work:Math.max(left.work,right.work)+1}],total+cost);
  }
 }visit(values,0);return best;
}
test('optimizer matches independent exhaustive pair-reduction oracle on 40 bounded cases',()=>{
 const choices=[book('sharpness',5),book('looting',3),book('unbreaking',3),book('mending',1)];
 for(let seed=0;seed<40;seed++){
  const input=request(choices.slice(0,2+seed%3).map((b,i)=>({...b,level:Math.max(1,b.level-(seed+i)%2),work:(seed>>i)%3})),{work:seed%3});
  assert.equal(solve(input).root.total,oracle(input),JSON.stringify(input));
 }
});
test('eight-book bound remains feasible; outputs never lose supported final enchantments',()=>{
 const input=request(['protection','feather_falling','depth_strider','soul_speed','thorns','unbreaking','mending','vanishing_curse'].map(id=>book(id,byId.get(id)!.max)),{item:'boots'});
 const result=solve(input);assert.equal(result.root.enchants.length,8);assert.equal(steps(result.root).length,8);assert.ok(result.visited<3_000_000);
 const walk=(node:Node):void=>{if(node.leaf===null){assert.ok(node.cost!<40);walk(node.left!);walk(node.right!);}};walk(result.root);
 assert.equal(applicable('mace').length,0);
});
test('cooperative solver matches synchronous results and cancellation interrupts a live search',async()=>{
 const input=request(['protection','feather_falling','depth_strider','soul_speed','thorns','unbreaking','mending','vanishing_curse'].map(id=>book(id,byId.get(id)!.max)),{item:'boots'});
 const abort=new AbortController(),pending=solveAsync(input,abort.signal);abort.abort();await assert.rejects(pending,/CANCELLED/);
 const result=await solveAsync(input,new AbortController().signal);assert.equal(result.root.total,solve(input).root.total);
});
