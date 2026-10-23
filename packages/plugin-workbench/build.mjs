// SPDX-License-Identifier: MIT
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
const require=createRequire(new URL('../../apps/desktop/package.json',import.meta.url));
const {build}=await import(pathToFileURL(require.resolve('vite')).href);
const id=process.argv[2];
if(!id||!/^[a-z][a-z0-9-]{1,60}$/.test(id))throw Error('Usage: node packages/plugin-workbench/build.mjs <plugin-id>');
const root=fileURLToPath(new URL(`../../examples/${id}/ui/`,import.meta.url));
await build({configFile:false,root,base:'./',build:{outDir:path.join(root,'dist'),emptyOutDir:true,sourcemap:false,target:['es2022','chrome105','safari15']}});
