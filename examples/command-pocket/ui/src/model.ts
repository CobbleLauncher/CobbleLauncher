// SPDX-License-Identifier: MIT
export type Entry = { id:string; name:string; template:string; favorite:boolean };
export type Book = {schemaVersion:1; entries:Entry[]};
export const key='commands-v1';
/** Session-only input recall; templates and server contexts never share parameters. */
export class ParameterMemory {
 private readonly entries=new Map<string,Record<string,string>>();
 private id(scope:unknown,entry:Pick<Entry,'id'|'template'>){return JSON.stringify([scope,entry.id,entry.template]);}
 recall(scope:unknown,entry:Pick<Entry,'id'|'template'>){return {...this.entries.get(this.id(scope,entry))};}
 remember(scope:unknown,entry:Pick<Entry,'id'|'template'>,values:Record<string,string>){
  const id=this.id(scope,entry);this.entries.delete(id);this.entries.set(id,{...values});
  while(this.entries.size>64)this.entries.delete(this.entries.keys().next().value!);
 }
}
export function parameters(template:string):string[]{
 if(!/^\/\S+/.test(template)||template.length>4096||/[\r\n\u0000-\u001f\u007f]/.test(template))throw Error('INVALID_TEMPLATE');
 const names=[...template.matchAll(/\{([a-zA-Z][a-zA-Z0-9_]{0,31})\}/g)].map(match=>match[1]!);
 if(/[{}]/.test(template.replace(/\{([a-zA-Z][a-zA-Z0-9_]{0,31})\}/g,''))||new Set(names).size>16)throw Error('INVALID_TEMPLATE');
 return [...new Set(names)];
}
export function fill(template:string,values:Record<string,string>):string{
 const names=parameters(template);for(const name of names){const value=values[name];if(!value?.trim()||value.length>256||/[\r\n\u0000-\u001f\u007f]/.test(value))throw Error('MISSING_PARAMETER');}
 const result=template.replace(/\{([a-zA-Z][a-zA-Z0-9_]{0,31})\}/g,(_,name)=>values[name]!);if(result.length>8192)throw Error('COMMAND_TOO_LONG');return result;
}
export function decode(value:unknown):Book{
 if(value===null)return{schemaVersion:1,entries:[]};const book=value as Book;
 if(book?.schemaVersion!==1||!Array.isArray(book.entries)||book.entries.length>200)throw Error('INVALID_BOOK');
 const ids=new Set<string>();for(const entry of book.entries){if(!entry||typeof entry.id!=='string'||!entry.id||entry.id.length>80||ids.has(entry.id)||typeof entry.name!=='string'||!entry.name.trim()||entry.name.length>80||/[\u0000-\u001f\u007f]/.test(entry.name)||typeof entry.favorite!=='boolean'||typeof entry.template!=='string')throw Error('INVALID_BOOK');parameters(entry.template);ids.add(entry.id);}return book;
}
export function generate(kind:string,values:Record<string,string>):string{
 if(!['1.20.1','1.21.1','1.21.4'].includes(values.version!))throw Error('UNSUPPORTED_VERSION');
 if(!/^(?:@[pas]|[A-Za-z0-9_]{1,16})$/.test(values.target!))throw Error('INVALID_TARGET');
 const integer=(name:string,min:number,max:number)=>{if(!/^\d+$/.test(values[name]!)||Number(values[name]!)<min||Number(values[name]!)>max)throw Error('INVALID_NUMBER');return Number(values[name]!);};
 const resource=(name:string)=>{if(values[name]!.length>128||!/^[a-z0-9_.-]+:[a-z0-9_./-]+$/.test(values[name]!))throw Error('INVALID_RESOURCE');return values[name]!;};
 if(kind==='teleport'){for(const name of ['x','y','z'])if(!/^(?:[~^](?:-?\d+(?:\.\d+)?)?|-?\d+(?:\.\d+)?)$/.test(values[name]!)||values[name]!.length>32)throw Error('INVALID_COORDINATE');if(['x','y','z'].some(n=>values[n]!.startsWith('^'))&&!['x','y','z'].every(n=>values[n]!.startsWith('^')))throw Error('MIXED_COORDINATES');return `/tp ${values.target} ${values.x} ${values.y} ${values.z}`;}
 if(kind==='give')return `/give ${values.target} ${resource('item_id')} ${integer('quantity',1,64)}`;
 if(kind==='effect'&&!['true','false'].includes(values.hide_particles??''))throw Error('INVALID_BOOLEAN');
 if(kind==='effect')return `/effect give ${values.target} ${resource('effect_id')} ${integer('seconds',1,1000000)} ${integer('amplifier',0,127)} ${values.hide_particles==='true'}`;
 throw Error('INVALID_COMMAND');
}



