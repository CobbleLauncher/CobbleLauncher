// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { releasePlan } from './publish-updater-release.mjs';

test('stable and preview feeds are independent and packages use immutable version URLs', () => {
 const base = {repository:'CobbleLauncher/CobbleLauncher',version:'1.2.3',installerName:'Cobble Launcher_1.2.3_x64-setup.exe'};
 const stable=releasePlan({...base,channel:'stable'}),preview=releasePlan({...base,channel:'preview'});
 assert.equal(stable.channelTag,'update-stable');
 assert.equal(preview.channelTag,'update-preview');
 assert.equal(stable.prerelease,false); assert.equal(preview.prerelease,true);
 assert.match(stable.url,/\/v1\.2\.3\/Cobble%20Launcher/);
 assert.throws(()=>releasePlan({...base,channel:'stable',version:'1.2.3-beta.1'}));
 assert.throws(()=>releasePlan({...base,channel:'oops'}));
 assert.throws(()=>releasePlan({...base,channel:'preview',installerName:'../other.exe'}));
});

import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { publish } from './publish-updater-release.mjs';

test('failed version upload cannot publish or advance the channel', () => {
 const directory=mkdtempSync(path.join(tmpdir(),'cobble-release-'));
 try {
  writeFileSync(path.join(directory,'setup.exe'),'synthetic installer');
  writeFileSync(path.join(directory,'setup.exe.sig'),'synthetic signature');
  const calls=[];
  assert.throws(()=>publish(directory,{GITHUB_REPOSITORY:'CobbleLauncher/CobbleLauncher',GITHUB_SHA:'a'.repeat(40),COBBLE_RELEASE_CHANNEL:'preview'},(_cmd,args)=>{
   calls.push(args);
   if(args[1]==='view')return {status:1,stderr:'release not found'};
   if(args[1]==='upload')return {status:1,stderr:'Upload failed'};
   return {status:0,stdout:''};
  }),/Upload failed/);
  assert.equal(calls.some(args=>args[1]==='edit'),false);
  assert.equal(calls.some(args=>args.includes('update-preview')),false);
 } finally {rmSync(directory,{recursive:true,force:true});}
});
