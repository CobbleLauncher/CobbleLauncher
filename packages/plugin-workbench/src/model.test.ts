// SPDX-License-Identifier: MIT
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {columns,datasets,filterRows,format,localized,lookup} from './model.ts';
test('snapshot data retains domain collections and searchable details',()=>{
 const data={count:2,mods:[{id:'one',name:'First',size:2048},{id:'two',name:'Second',enabled:false}]};
 assert.equal(datasets(data)[0]?.key,'mods');assert.equal(filterRows(data.mods,'SECOND').length,1);assert.equal(lookup({nested:{size:9}},'nested.size'),9);
 assert.deepEqual(columns(data.mods).map(field=>field.key),['name','size','enabled']);
});
test('values are readable and missing values are explicit',()=>{assert.equal(format(2048,'bytes'),'2.0 KiB');assert.equal(format(false,undefined,true),'否');assert.equal(format(null),'—');assert.equal(localized({en:'Name','zh-CN':'名称'},true),'名称');assert.equal(datasets(null).length,0);});
test('native alphabetical records lead with names and leave structured metadata in details',()=>{const rows=[{declaredIds:['mod'],dependencies:[{id:'required'}],disabled:false,loader:'Fabric',name:'Mod name',version:'1.0.0'}];assert.deepEqual(columns(rows).map(field=>field.key),['name','disabled','loader','version']);assert.equal(rows[0]?.dependencies[0]?.id,'required');});
