// SPDX-License-Identifier: MIT
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, rmdirSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { prepareFrontendInput, replaceGeneratedPackage } from '../../scripts/build-plugin.mjs';
const project = dirname(fileURLToPath(import.meta.url)), root = resolve(project, '../..'), target = join(root, 'target');
const run = (command, args, options = {}) => execFileSync(command, args, { cwd: root, stdio: 'inherit', ...options });
const tools = process.env.COBBLE_WASM_TOOLS || 'wasm-tools';
const version = execFileSync(tools, ['--version'], { encoding:'utf8' });
if (!/^wasm-tools 1\.262\.0\b/.test(version)) throw Error('Use pinned wasm-tools 1.262.0.');
mkdirSync(target, { recursive:true });
run('cargo', ['build','--locked','--manifest-path',join(project,'Cargo.toml'),'--release','--target','wasm32-unknown-unknown','--target-dir',join(target,'mesh-events-demo-build')]);
const raw = join(target,'mesh-events-demo-build/wasm32-unknown-unknown/release/cobble_mesh_events_demo.wasm'), component = join(target,'mesh-events-demo.component.wasm');
run(tools, ['component','new',raw,'-o',component]); run(tools, ['validate',component]);
run(process.execPath, [join(root,'packages/plugin-workbench/build.mjs'),'mesh-events-demo']);
const manifestPath = join(project,'plugin.json'), manifest = JSON.parse(readFileSync(manifestPath,'utf8'));
manifest.component_sha256 = createHash('sha256').update(readFileSync(component)).digest('hex');
writeFileSync(manifestPath,JSON.stringify(manifest,null,2)+'\n');
function emptyTemp(prefix) { const path = mkdtempSync(join(target,prefix)); rmdirSync(path); return path; }
function cleanup(path) {
  const inside = relative(target,resolve(path));
  if (!inside || inside === '..' || inside.startsWith('..'+sep) || resolve(path) === target) throw Error('Unexpected generated cleanup path.');
  if (existsSync(path)) rmSync(path,{recursive:true,force:true});
}
const input = emptyTemp('mesh-events-demo-input-'), staging = emptyTemp('mesh-events-demo-package-');
try {
  const prepared = prepareFrontendInput(manifestPath,join(project,'ui/dist'),input);
  writeFileSync(manifestPath,readFileSync(prepared));
  run('cargo',['run','--locked','-p','cobble-plugin-sdk','--','pack',component,prepared,staging]);
  const output = join(project,'package'); replaceGeneratedPackage(staging,output);
  run('cargo',['run','--locked','-p','cobble-plugin-sdk','--','check',output]);
  console.log('Developer example package: '+output);
} finally { cleanup(input); cleanup(staging); }
