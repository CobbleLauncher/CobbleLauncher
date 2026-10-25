// SPDX-License-Identifier: MIT
export type Declaration = {id:string;version:string|null;optional:boolean};
export type Mod = {id:string;name:string;version:string|null;loader:string|null;disabled:boolean;declaredIds:string[];dependencies:Declaration[]};
export type Inventory = {items:Mod[];truncated:boolean;available:boolean};
export type FactStatus = 'present'|'disabled'|'missing'|'runtime'|'ambiguous';
export type Fact = Declaration & {source:string;targets:string[];status:FactStatus};
export type GraphNode = {id:string;name:string;mod:Mod|null;kind:'mod'|'missing'|'runtime'};
export type Graph = {nodes:Map<string,GraphNode>;facts:Fact[];outgoing:Map<string,Fact[]>;incoming:Map<string,Fact[]>};
export type Filter = 'all'|'enabled'|'disabled'|'requirements'|'unknown';
const runtimeIds = new Set(['minecraft','java','fabricloader','quilt_loader','forge','neoforge']);
const object = (value:unknown):Record<string,unknown> => value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Record<string,unknown> : {};
const text = (value:unknown):value is string => typeof value === 'string' && value.length > 0 && value.length <= 256;
export function parseInventory(value:unknown):Inventory {
  const data = object(value);
  if (!Array.isArray(data.items) || data.items.length > 512) throw Error('INVALID_INVENTORY');
  const ids = new Set<string>();
  const items = data.items.map(value => {
    const item = object(value);
    if (!text(item.id) || !text(item.name) || ids.has(item.id) || typeof item.disabled !== 'boolean' || !Array.isArray(item.declaredIds) || item.declaredIds.length > 65 || !item.declaredIds.every(text) || !Array.isArray(item.dependencies) || item.dependencies.length > 64) throw Error('INVALID_MOD');
    ids.add(item.id);
    const nullable = (v:unknown) => v === null || v === undefined ? null : text(v) ? v : (() => {throw Error('INVALID_METADATA');})();
    const dependencies = item.dependencies.map(value => {
      const dependency = object(value);
      if (!text(dependency.id) || typeof dependency.optional !== 'boolean') throw Error('INVALID_DEPENDENCY');
      return {id:dependency.id,version:nullable(dependency.version),optional:dependency.optional};
    });
    return {id:item.id,name:item.name,disabled:item.disabled,declaredIds:item.declaredIds as string[],dependencies,version:nullable(item.version),loader:nullable(item.loader)};
  });
  return {items,truncated:data.truncated === true,available:data.available !== false};
}
export const modKey = (id:string) => `mod:${id}`;
export function dependencyGraph(items:readonly Mod[]):Graph {
  const nodes = new Map<string,GraphNode>(), providers = new Map<string,Mod[]>(), facts:Fact[] = [], outgoing = new Map<string,Fact[]>(), incoming = new Map<string,Fact[]>();
  for (const mod of items) {
    nodes.set(modKey(mod.id),{id:modKey(mod.id),name:mod.name,mod,kind:'mod'});
    for (const id of new Set(mod.declaredIds)) providers.set(id,[...(providers.get(id) ?? []),mod]);
  }
  for (const mod of items) for (const declared of mod.dependencies) {
    const candidates = providers.get(declared.id) ?? [], enabled = candidates.filter(item => !item.disabled);
    const status:FactStatus = enabled.length > 1 ? 'ambiguous' : enabled.length ? 'present' : candidates.length ? 'disabled' : runtimeIds.has(declared.id) ? 'runtime' : 'missing';
    const targets = (enabled.length ? enabled : candidates).map(item => modKey(item.id));
    if (!targets.length) {
      const id = `${status}:${declared.id}`;
      nodes.set(id,{id,name:declared.id,mod:null,kind:status === 'runtime' ? 'runtime' : 'missing'});
      targets.push(id);
    }
    const fact = {...declared,source:modKey(mod.id),targets,status};
    facts.push(fact); outgoing.set(fact.source,[...(outgoing.get(fact.source) ?? []),fact]);
    for (const target of targets) incoming.set(target,[...(incoming.get(target) ?? []),fact]);
  }
  return {nodes,facts,outgoing,incoming};
}
export function filterMods(graph:Graph,query:string,filter:Filter):GraphNode[] {
  const search = query.trim().toLocaleLowerCase();
  return [...graph.nodes.values()].filter(node => {
    const mod = node.mod;
    if (!mod) return false;
    if (filter === 'enabled' && mod.disabled || filter === 'disabled' && !mod.disabled || filter === 'unknown' && mod.declaredIds.length && mod.version) return false;
    if (filter === 'requirements' && !graph.outgoing.get(node.id)?.some(fact => !fact.optional && ['missing','disabled'].includes(fact.status))) return false;
    return !search || [mod.name,mod.id,mod.version ?? '',...mod.declaredIds,...mod.dependencies.map(dep => dep.id)].some(value => value.toLocaleLowerCase().includes(search));
  });
}
export function selectedNode(graph:Graph,visible:readonly GraphNode[],previous:string|null):string|null {
  return previous && graph.nodes.has(previous) && (visible.some(node => node.id === previous) || !graph.nodes.get(previous)?.mod) ? previous : visible[0]?.id ?? null;
}
/** Keep graph work bounded even for dense inventories; the list retains every Mod. */
export function boundedNeighborhood(graph:Graph,seeds:readonly string[],limit=40):{nodes:GraphNode[];facts:Fact[];omitted:number;totalNodes:number;omittedEdges:number;edgeCount:number} {
  const budget = Math.max(1,Math.min(160,Math.floor(limit))), ids = new Set<string>();
  const reachable = new Set<string>(), reachQueue = [...new Set(seeds)].filter(id=>graph.nodes.has(id)), reachQueued=new Set(reachQueue);
  for(let cursor=0;cursor<reachQueue.length;cursor++) {
    const id=reachQueue[cursor]!;if(reachable.has(id))continue;reachable.add(id);
    const adjacent=[...(graph.outgoing.get(id) ?? []).flatMap(fact=>fact.targets),...(graph.incoming.get(id) ?? []).map(fact=>fact.source)];
    for(const next of adjacent)if(!reachQueued.has(next)){reachQueued.add(next);reachQueue.push(next);}
  }
  for (const seed of new Set(seeds)) {
    if (!graph.nodes.has(seed) || ids.size >= budget) continue;
    const queue = [seed], queued = new Set(queue);
    for (let cursor=0;cursor<queue.length && ids.size<budget;cursor++) {
      const id = queue[cursor]!;
      if (ids.has(id)) continue;
      ids.add(id);
      const adjacent = [...(graph.outgoing.get(id) ?? []).flatMap(fact=>fact.targets),...(graph.incoming.get(id) ?? []).map(fact=>fact.source)];
      for (const next of adjacent) if (!queued.has(next) && !ids.has(next)) {queued.add(next);queue.push(next);}
    }
  }
  const candidates = graph.facts.filter(fact => ids.has(fact.source) && fact.targets.some(target => ids.has(target)));
  const edgeBudget = budget*4, facts:Fact[] = []; let totalEdges=0,edgeCount=0;
  for (const fact of candidates) {
    const targets = fact.targets.filter(target=>ids.has(target));totalEdges+=targets.length;
    const boundedTargets = targets.slice(0,Math.max(0,edgeBudget-edgeCount));
    if (boundedTargets.length) {facts.push({...fact,targets:boundedTargets});edgeCount+=boundedTargets.length;}
  }
  return {nodes:[...ids].map(id => graph.nodes.get(id)!),facts,omitted:reachable.size-ids.size,totalNodes:reachable.size,omittedEdges:totalEdges-edgeCount,edgeCount};
}
