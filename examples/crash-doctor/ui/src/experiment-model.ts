// SPDX-License-Identifier: MIT
import type { ModInventory, DiagnosticDocument, Report } from '../../../../packages/plugin-web-sdk/src/index';
import type { RecoverySnapshot } from '../../../../packages/plugin-web-sdk/src/recovery-workspace';
export type Mod = ModInventory['items'][number];
export type ExperimentResult = 'sameFailure' | 'missingDependency' | 'needsWorld' | 'success';
export type ExperimentRecord = { id:string; at:number; copyGameId:string; taskId:string|null; mods:{id:string;name:string}[]; requested:string[]; includeWorlds:boolean; baselineAt:number|null; failureAt:number|null; diagnostic:{id:string;revision:string}|null; status:'copying'|'pending'|'ready'|'failed'|'cleaned'; result:ExperimentResult|null };
export type ExperimentState = { schemaVersion:1; rounds:ExperimentRecord[] };
export type Suggestion = { id:string; roots:string[]; mods:Mod[]; evidence:('log'|'change')[]; cycle:boolean };
export function requiredGraph(mods:Mod[]) {
 const enabled=mods.filter(mod=>!mod.disabled), graph=new Map(enabled.map(mod=>[mod.id,[] as string[]]));
 for(const mod of enabled)for(const dependency of mod.dependencies.filter(dep=>!dep.optional))for(const provider of enabled.filter(candidate=>candidate.declaredIds.includes(dependency.id)))graph.get(mod.id)!.push(provider.id);
 return graph;
}
export function dependentClosure(mods:Mod[], roots:string[]) {
 const graph=requiredGraph(mods), selected=new Set(roots.filter(id=>graph.has(id)));
 let changed=true;while(changed){changed=false;for(const [id,deps]of graph)if(!selected.has(id)&&deps.some(dep=>selected.has(dep))){selected.add(id);changed=true;}}
 return mods.filter(mod=>selected.has(mod.id));
}
/** Tarjan SCCs prevent mutually required Mods from becoming separate experiment roots. */
export function components(mods:Mod[]) {
 const graph=requiredGraph(mods),indices=new Map<string,number>(),low=new Map<string,number>(),stack:string[]=[],onStack=new Set<string>(),groups:string[][]=[];let next=0;
 function visit(id:string){indices.set(id,next);low.set(id,next++);stack.push(id);onStack.add(id);for(const dep of graph.get(id)??[]){if(!indices.has(dep)){visit(dep);low.set(id,Math.min(low.get(id)!,low.get(dep)!));}else if(onStack.has(dep))low.set(id,Math.min(low.get(id)!,indices.get(dep)!));}if(low.get(id)===indices.get(id)){const group:string[]=[];let member:string;do{member=stack.pop()!;onStack.delete(member);group.push(member);}while(member!==id);groups.push(group.sort());}}
 for(const id of graph.keys())if(!indices.has(id))visit(id);return groups;
}
export function suggestions(mods:Mod[], snapshot:RecoverySnapshot|null, document:DiagnosticDocument|null, report:Report|null):Suggestion[] {
 const text=[document?.text??'',...(report?.findings??[]).map(finding=>`${finding.title} ${finding.message}`)].join('\n').toLowerCase();
 const changed=new Set(snapshot?.comparison.available?snapshot.comparison.modOperations.flatMap(operation=>operation.files.flatMap(file=>[file.after,file.before].filter((id):id is string=>!!id))):[]);
 const candidates=components(mods).map(roots=>{const members=mods.filter(mod=>roots.includes(mod.id));const evidence:Suggestion['evidence']=[];if(members.some(mod=>[mod.id,...mod.declaredIds].some(id=>id.length>2&&text.includes(id.toLowerCase()))))evidence.push('log');if(members.some(mod=>changed.has(mod.id)))evidence.push('change');return {id:roots.join('|'),roots,mods:dependentClosure(mods,roots),evidence,cycle:roots.length>1};});
 return candidates.sort((a,b)=>b.evidence.length-a.evidence.length||Number(b.evidence.includes('log'))-Number(a.evidence.includes('log'))||a.mods.length-b.mods.length||a.id.localeCompare(b.id)).slice(0,32);
}
export function packedState(state:ExperimentState) {
 const mods:{id:string;name:string}[]=[],indices=new Map<string,number>();
 const rounds=state.rounds.map(round=>({...round,mods:round.mods.map(mod=>{let index=indices.get(`${mod.id}\u0000${mod.name}`);if(index===undefined){index=mods.length;mods.push(mod);indices.set(`${mod.id}\u0000${mod.name}`,index);}return index;})}));
 const packed={schemaVersion:2,mods,rounds};
 if(new TextEncoder().encode(JSON.stringify(packed)).length>60*1024)throw Error('EXPERIMENT_STATE_LIMIT');
 return packed;
}
export function parseState(value:unknown):ExperimentState {
 if(value===null)return {schemaVersion:1,rounds:[]};
 if(value&&typeof value==='object'&&(value as {schemaVersion?:unknown}).schemaVersion===2){const packed=value as {mods?:{id:string;name:string}[];rounds?:Array<Omit<ExperimentRecord,'mods'>&{mods:number[]}>};if(!Array.isArray(packed.mods)||packed.mods.length>4096||!Array.isArray(packed.rounds)||packed.rounds.length>32)throw Error('EXPERIMENT_STATE_INVALID');const dictionary=packed.mods;value={schemaVersion:1,rounds:packed.rounds.map(round=>({...round,mods:Array.isArray(round.mods)?round.mods.map(index=>Number.isInteger(index)&&index>=0?dictionary[index]:undefined):null}))};}
 if(!value||typeof value!=='object'||(value as ExperimentState).schemaVersion!==1||!Array.isArray((value as ExperimentState).rounds)||(value as ExperimentState).rounds.length>32)throw Error('EXPERIMENT_STATE_INVALID');
 const source=value as ExperimentState;
 for(const round of source.rounds){if(!round||typeof round.id!=='string'||!Number.isSafeInteger(round.at)||!Array.isArray(round.mods)||!round.mods.length||round.mods.length>512||!round.mods.every(mod=>mod&&typeof mod.id==='string'&&typeof mod.name==='string')||!Array.isArray(round.requested)||!round.requested.every(id=>typeof id==='string')||typeof round.copyGameId!=='string'||round.taskId!==null&&typeof round.taskId!=='string'||typeof round.includeWorlds!=='boolean'||!['copying','pending','ready','failed','cleaned'].includes(round.status)||round.result!==null&&!['sameFailure','missingDependency','needsWorld','success'].includes(round.result))throw Error('EXPERIMENT_STATE_INVALID');}
 return structuredClone(source);
}
export function nextSuggestion(candidates:Suggestion[], state:ExperimentState) {
 const previous=state.rounds.at(-1);
 // A dependency failure or missing in-world test is inconclusive; retain its group.
 if(previous&&(previous.result==='missingDependency'||previous.result==='needsWorld'))return candidates.find(candidate=>candidate.roots.join('|')===previous.requested.join('|'))??candidates[0]??null;
 const tested=new Set(state.rounds.filter(round=>round.result==='sameFailure'||round.result==='success').map(round=>round.requested.join('|')));
 return candidates.find(candidate=>!tested.has(candidate.roots.join('|')))??null;
}
