// SPDX-License-Identifier: MIT
import {test} from 'node:test';import assert from 'node:assert/strict';import {canExport} from './model.ts';
test('sharing requires reviewed configuration and explicit excluded-mod acknowledgment',()=>{const p={excludedMods:['unknown.jar']} as Parameters<typeof canExport>[0];assert.equal(canExport(p,false,true,false),false);assert.equal(canExport(p,false,false,true),false);assert.equal(canExport(p,true,true,true),false);assert.equal(canExport(p,false,true,true),true);assert.equal(canExport(null,false,true,true),false);});
