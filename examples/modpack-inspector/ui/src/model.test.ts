// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import {boundedNeighborhood,dependencyGraph,filterMods,modKey,parseInventory,selectedNode,type Mod} from './model.ts';
const mod=(id:string,patch:Partial<Mod>={}):Mod=>({id:`${id}.jar`,name:id,version:'1.0',loader:'fabric',disabled:false,declaredIds:[id],dependencies:[],...patch});
test('ID matching never claims version-range compatibility and preserves unknown declarations',()=>{
  const graph=dependencyGraph([mod('main',{dependencies:[{id:'helper',version:'>=99',optional:false},{id:'java',version:null,optional:false},{id:'absent',version:'*',optional:true}]}),mod('helper')]);
  assert.equal(graph.facts[0].status,'present');assert.equal(graph.facts[0].version,'>=99');assert.equal(graph.facts[1].status,'runtime');assert.equal(graph.facts[1].version,null);assert.equal(graph.facts[2].status,'missing');assert.equal(graph.facts[2].optional,true);
  assert.equal(graph.incoming.get(modKey('helper.jar'))?.[0].source,modKey('main.jar'));
});
test('disabled providers, aliases and duplicates stay distinguishable',()=>{
  const main=mod('main',{dependencies:[{id:'alias',version:'*',optional:false}]}),disabled=mod('one',{disabled:true,declaredIds:['alias']});
  assert.equal(dependencyGraph([main,disabled]).facts[0].status,'disabled');
  const duplicate=dependencyGraph([main,mod('one',{declaredIds:['alias']}),mod('two',{declaredIds:['alias']})]);
  assert.equal(duplicate.facts[0].status,'ambiguous');assert.equal(duplicate.facts[0].targets.length,2);
});
test('search includes declared IDs and requirements; filters never hide inventory facts',()=>{
  const graph=dependencyGraph([mod('读图器',{dependencies:[{id:'unavailable-api',version:null,optional:false}]}),mod('off',{disabled:true}),mod('unknown',{version:null,declaredIds:[]})]);
  assert.deepEqual(filterMods(graph,'API','requirements').map(node=>node.name),['读图器']);assert.equal(filterMods(graph,'','disabled').length,1);assert.equal(filterMods(graph,'','unknown').length,1);assert.equal(filterMods(graph,'读图','all').length,1);
});
test('dense cyclic graphs remain bounded, include selected neighbors and expand deterministically',()=>{
  const graph=dependencyGraph(Array.from({length:200},(_,i)=>mod(String(i),{dependencies:[{id:String((i+1)%200),version:'*',optional:false}]})));
  const initial=boundedNeighborhood(graph,[modKey('120.jar')]);assert.equal(initial.nodes.length,40);assert.equal(initial.nodes[0].id,modKey('120.jar'));assert(initial.nodes.some(node=>node.id===modKey('121.jar')));
  const expanded=boundedNeighborhood(graph,[modKey('120.jar')],80);assert.equal(expanded.nodes.length,80);assert.equal(boundedNeighborhood(graph,[modKey('120.jar')],10000).nodes.length,160);assert.equal(initial.omitted,160);
  assert(initial.facts.every(fact=>initial.nodes.some(node=>node.id===fact.source)&&fact.targets.some(id=>initial.nodes.some(node=>node.id===id))));
});
test('selection survives refresh by ID and falls back when a filter removes it',()=>{
  const graph=dependencyGraph([mod('a'),mod('b',{disabled:true})]),all=filterMods(graph,'','all');assert.equal(selectedNode(graph,all,modKey('b.jar')),modKey('b.jar'));assert.equal(selectedNode(graph,filterMods(graph,'','enabled'),modKey('b.jar')),modKey('a.jar'));assert.equal(selectedNode(graph,[],null),null);
});
test('dense graphs also bound layout edge work while details retain every declaration',()=>{
  const graph=dependencyGraph(Array.from({length:80},(_,i)=>mod(String(i),{dependencies:Array.from({length:64},(_,j)=>({id:String(j),version:'*',optional:false}))})));
  const bounded=boundedNeighborhood(graph,[modKey('0.jar')]);assert.equal(bounded.nodes.length,40);assert.equal(bounded.edgeCount,160);assert(bounded.omittedEdges>1000);assert.equal(graph.outgoing.get(modKey('0.jar'))?.length,64);
});
test('neighborhood never fills spare graph space with disconnected inventory nodes',()=>{
  const graph=dependencyGraph([mod('main',{dependencies:[{id:'helper',version:'*',optional:false}]}),mod('helper'),mod('unrelated')]);
  const bounded=boundedNeighborhood(graph,[modKey('main.jar')]);assert.equal(bounded.nodes.length,2);assert.equal(bounded.totalNodes,2);assert.equal(bounded.omitted,0);assert(!bounded.nodes.some(node=>node.name==='unrelated'));
});
test('snapshot parser rejects malformed and over-budget input instead of fabricating metadata',()=>{
  assert.equal(parseInventory({items:[mod('valid')]}).items[0].version,'1.0');assert.throws(()=>parseInventory({items:[mod('x'),mod('x')]}));assert.throws(()=>parseInventory({items:[{...mod('x'),dependencies:[{id:'x',version:'*'}]}]}));assert.throws(()=>parseInventory({items:Array.from({length:513},(_,i)=>mod(String(i)))}));
});
