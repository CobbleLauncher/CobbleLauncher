// SPDX-License-Identifier: MIT
// Original subset dynamic programming. No upstream optimizer implementation adapted.
import {byId,equipment} from './catalog.ts';
export type Enchant={id:string;level:number};
export type Book=Enchant & {work:number};
export type Request={version:string;item:string;work:number;existing:Enchant[];books:Book[];creative:boolean};
export type Node={enchants:Enchant[];work:number;total:number;peak:number;leaf:number|null;left?:Node;right?:Node;cost?:number};
export type Result={root:Node;survival:boolean;visited:number};
export const penalty=(work:number)=>2**work-1;
const vector=(enchants:Enchant[])=>enchants.map(e=>`${e.id}:${e.level}`).join(',');
export function validate(request:Request):void {
 if(!['1.20.1','1.21.1'].includes(request.version))throw Error('VERSION_UNSUPPORTED');
 if(!equipment[request.item])throw Error('ITEM_UNSUPPORTED');
 if(!Number.isInteger(request.work)||request.work<0||request.work>6)throw Error('WORK_INVALID');
 if(!Array.isArray(request.books)||request.books.length<1||request.books.length>8||!Array.isArray(request.existing)||request.existing.length>16)throw Error('BOOK_LIMIT');
 const existing=new Set<string>();
 for(const enchant of [...request.existing,...request.books]) {
  const data=byId.get(enchant.id);
  if(!data?.items.includes(request.item))throw Error('ENCHANT_UNSUPPORTED');
  if(!Number.isInteger(enchant.level)||enchant.level<1||enchant.level>data.max)throw Error('LEVEL_INVALID');
 }
 for(const enchant of request.existing){if(existing.has(enchant.id))throw Error('EXISTING_DUPLICATE');existing.add(enchant.id);}
 for(const book of request.books)if(!Number.isInteger(book.work)||book.work<0||book.work>6)throw Error('WORK_INVALID');
 const ids=new Set([...request.existing,...request.books].map(e=>e.id));
 for(const id of ids)if(byId.get(id)!.conflicts.some(other=>ids.has(other)))throw Error('ENCHANT_CONFLICT');
}
export function merge(left:Node,right:Node):Node {
 const levels=new Map(left.enchants.map(e=>[e.id,e.level]));let cost=penalty(left.work)+penalty(right.work);
 for(const enchant of right.enchants){
  const data=byId.get(enchant.id)!;const before=levels.get(enchant.id)??0;
  const after=before===enchant.level?Math.min(data.max,before+1):Math.max(before,enchant.level);
  levels.set(enchant.id,after);cost+=data.weight*after;
 }
 return {enchants:[...levels].sort(([a],[b])=>a.localeCompare(b)).map(([id,level])=>({id,level})),work:Math.max(left.work,right.work)+1,total:left.total+right.total+cost,peak:Math.max(left.peak,right.peak,cost),leaf:null,left,right,cost};
}
function desired(request:Request):string {
 const counts=new Map<string,number[]>();
 for(const e of [...request.existing,...request.books]){const arr=counts.get(e.id)??Array(6).fill(0) as number[];arr[e.level]=(arr[e.level]??0)+1;counts.set(e.id,arr);}
 const result:Enchant[]=[];
 for(const [id,arr]of counts){const max=byId.get(id)!.max;for(let level=1;level<max;level++)arr[level+1]=(arr[level+1]??0)+Math.floor((arr[level]??0)/2);result.push({id,level:Math.max(...arr.map((count,i)=>count?i:0))});}
 return vector(result.sort((a,b)=>a.id.localeCompare(b.id)));
}
function* computation(request:Request,limit:number):Generator<void,Result> {
 validate(request);let visited=0;const desiredVector=desired(request);
 function* search(cap:number):Generator<void,Node|null> {
  const size=1<<(request.books.length+1);const states:Array<Map<string,Node>>=Array.from({length:size},()=>new Map());
  const leaf=(enchants:Enchant[],work:number,index:number):Node=>({enchants:[...enchants].sort((a,b)=>a.id.localeCompare(b.id)),work,total:0,peak:0,leaf:index});
  const initial=[leaf(request.existing,request.work,0),...request.books.map((book,i)=>leaf([{id:book.id,level:book.level}],book.work,i+1))];
  initial.forEach((node,index)=>states[1<<index]!.set(`${vector(node.enchants)}|${node.work}`,node));
  for(let mask=1;mask<size;mask++){
   if((mask&(mask-1))===0)continue;
   for(let leftMask=(mask-1)&mask;leftMask;leftMask=(leftMask-1)&mask){
    const rightMask=mask^leftMask;
    if(!rightMask||(rightMask&1))continue; // Equipment may only occupy the left slot.
    for(const left of states[leftMask]!.values())for(const right of states[rightMask]!.values()){
     if(++visited>limit)throw Error('SEARCH_LIMIT');
     if(visited%512===0)yield;
     const node=merge(left,right);if(node.cost!>cap)continue;
     const key=`${vector(node.enchants)}|${node.work}`;const previous=states[mask]!.get(key);
     if(!previous||node.total<previous.total||(node.total===previous.total&&node.peak<previous.peak))states[mask]!.set(key,node);
    }
   }
  }
  const finals=[...states[size-1]!.values()].filter(node=>vector(node.enchants)===desiredVector);
  finals.sort((a,b)=>a.total-b.total||a.work-b.work||a.peak-b.peak);return finals[0]??null;
 }
 const root=yield* search(request.creative?Infinity:39);
 if(root)return {root,survival:root.peak<40,visited};
 const fallback=yield* search(Infinity);if(!fallback)throw Error('NO_PLAN');
 return {root:fallback,survival:false,visited};
}
export function solve(request:Request,limit=3_000_000):Result {
 const run=computation(request,limit);let next=run.next();while(!next.done)next=run.next();return next.value;
}
/** Host plugin views disallow workers. Yield in bounded batches so input/cancel remains live. */
export async function solveAsync(request:Request,signal:AbortSignal,limit=3_000_000):Promise<Result>{
 const run=computation(request,limit);
 try{for(;;){
  if(signal.aborted)throw Error('CANCELLED');const deadline=performance.now()+6;
  do{const next=run.next();if(next.done)return next.value;}while(performance.now()<deadline);
  await new Promise<void>(resolve=>setTimeout(resolve,0));
 }}finally{run.return(undefined as never);}
}
export type Step={id:number;left:string;right:string;cost:number;penalty:number;enchants:Enchant[]};
export function steps(root:Node):Step[]{const result:Step[]=[];function walk(node:Node):string{if(node.leaf!==null)return node.leaf===0?'item':`book:${node.leaf}`;const left=walk(node.left!),right=walk(node.right!),id=result.length+1;result.push({id,left,right,cost:node.cost!,penalty:penalty(node.work),enchants:node.enchants});return `step:${id}`;}walk(root);return result;}
