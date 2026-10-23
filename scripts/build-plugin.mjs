// SPDX-License-Identifier: MIT
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, rmdirSync, unlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { officialPlugins, officialPluginIds } from './official-plugins.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const tools = process.env.COBBLE_WASM_TOOLS || 'wasm-tools';
const run = (exe, args, options = {}) => execFileSync(exe, args, { cwd: root, stdio: 'inherit', ...options });
const assetDigest = bytes => createHash('sha256').update(bytes).digest('hex');
function frontendPath(value) {
  return typeof value==='string' && value.length<=160 && value.startsWith('ui/') && /^[a-zA-Z0-9_./-]+$/.test(value) && value.split('/').every(p=>p && p!=='.' && p!=='..' && !p.endsWith('.') && !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\.|$)/i.test(p)) && /\.(html|js|mjs|css|json|png|jpg|jpeg|webp|woff2)$/.test(value);
}
function regularAsset(directory, name) {
  let current=directory;
  for(const part of name.split('/')) { current=path.join(current,part); if(lstatSync(current).isSymbolicLink())throw new Error('Frontend assets must not be links.'); }
  if(!lstatSync(current).isFile())throw new Error('Frontend asset must be a regular file.');
  return current;
}
function frontendFiles(directory, manifest) {
  if(!manifest.frontend)return [];
  const indexBytes=readFileSync(regularAsset(directory,'frontend-assets.json'));
  if(indexBytes.length>65536 || assetDigest(indexBytes)!==manifest.frontend.assets_sha256?.toLowerCase())throw new Error('Frontend index digest mismatch.');
  const index=JSON.parse(indexBytes);const seen=new Set();let total=0;
  if(index.schema_version!==1 || !Array.isArray(index.files) || !index.files.length || index.files.length>128)throw new Error('Invalid frontend index.');
  for(const file of index.files) {
    if(!frontendPath(file.path)||seen.has(file.path.toLowerCase())||!Number.isSafeInteger(file.bytes)||file.bytes<0||file.bytes>4194304)throw new Error('Invalid frontend asset.');seen.add(file.path.toLowerCase());total+=file.bytes;
    const source=regularAsset(directory,file.path);if(lstatSync(source).size!==file.bytes||assetDigest(readFileSync(source))!==file.sha256?.toLowerCase())throw new Error('Frontend asset digest mismatch.');
  }
  const entry=index.files.find(file=>file.path==='ui/index.html');
  if(manifest.frontend.entry!=='ui/index.html'||!entry||entry.bytes===0||entry.bytes>1048576||total>16777216)throw new Error('Frontend bundle exceeds budget or lacks entry.');
  return ['frontend-assets.json',...index.files.map(file=>file.path)];
}
export function prepareFrontendInput(manifestFile, dist, output) {
  const manifest=JSON.parse(readFileSync(manifestFile,'utf8'));if(!manifest.frontend)throw new Error('No frontend declared.');
  let total=0;const files=[];const walk=(directory,prefix='ui')=>{
    if(lstatSync(directory).isSymbolicLink())throw new Error('Frontend directories must not be links.');
    for(const item of readdirSync(directory,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))) {
      const source=path.join(directory,item.name),name=prefix+'/'+item.name;
      if(item.isSymbolicLink())throw new Error('Frontend assets must not be links.');
      if(item.isDirectory())walk(source,name);
      else {if(!item.isFile()||!frontendPath(name)||files.length>=128||lstatSync(source).size>4194304)throw new Error('Invalid frontend build output.');total+=lstatSync(source).size;if(total>16777216)throw new Error('Frontend bundle exceeds budget.');const bytes=readFileSync(source);files.push({path:name,sha256:assetDigest(bytes),bytes:bytes.length,source});}
    }
  };walk(dist);
  const indexBytes=Buffer.from(JSON.stringify({schema_version:1,files:files.map(({source,...file})=>file)},null,2)+'\n');
  manifest.frontend.assets_sha256=assetDigest(indexBytes);
  mkdirSync(output);for(const file of files){const target=path.join(output,file.path);mkdirSync(path.dirname(target),{recursive:true});copyFileSync(file.source,target);}
  writeFileSync(path.join(output,'frontend-assets.json'),indexBytes);
  writeFileSync(path.join(output,'plugin.json'),JSON.stringify(manifest,null,2)+'\n');
  if(manifest.icon){if(manifest.icon.file!=='icon.png')throw new Error('Unsupported declared plugin artwork.');copyFileSync(path.join(path.dirname(manifestFile),'icon.png'),path.join(output,'icon.png'));}
  frontendFiles(output,manifest);return path.join(output,'plugin.json');
}
export function replaceGeneratedPackage(staging, output) {
  if(path.resolve(staging)===path.resolve(output)||lstatSync(staging).isSymbolicLink()||(existsSync(output)&&lstatSync(output).isSymbolicLink()))throw new Error('Invalid generated staging directory.');
  const next = JSON.parse(readFileSync(path.join(staging, 'plugin.json'), 'utf8'));
  if (next.icon && next.icon.file !== 'icon.png') throw new Error('Unsupported declared plugin artwork.');
  const previousPath = path.join(output, 'plugin.json');
  const previous = existsSync(previousPath) ? JSON.parse(readFileSync(previousPath, 'utf8')) : null;
  mkdirSync(output, { recursive: true });
  const frontend = frontendFiles(staging,next);
  const previousFrontend=previous?.frontend ? frontendFiles(output,previous) : [];
  const files = ['component.wasm', ...(next.icon ? ['icon.png'] : []), ...frontend, 'plugin.json'];
  for (const name of files) { mkdirSync(path.dirname(path.join(output,name)),{recursive:true}); copyFileSync(path.join(staging, name), path.join(output, name)); unlinkSync(path.join(staging, name)); }
  if (previous?.icon?.file === 'icon.png' && !next.icon && existsSync(path.join(output, 'icon.png'))) unlinkSync(path.join(output, 'icon.png'));
  for(const name of previousFrontend) if(!frontend.includes(name) && existsSync(path.join(output,name)))unlinkSync(path.join(output,name));
  rmSync(staging,{recursive:true,force:true});
}
function main() {
let version;
try { version = run(tools, ['--version'], { stdio: 'pipe', encoding: 'utf8' }); }
catch { throw new Error('Install wasm-tools 1.262.0 with cargo install wasm-tools --version 1.262.0 --locked, or set COBBLE_WASM_TOOLS to its executable.'); }
if (!/^wasm-tools 1\.262\.0\b/.test(version)) throw new Error('This SDK requires wasm-tools 1.262.0.');
run('rustup', ['target', 'add', 'wasm32-unknown-unknown']);
const requested = process.argv.slice(2);
if (requested.some(id => !officialPluginIds.includes(id))) throw new Error('Unknown official plugin ID.');
const plugins = requested.length ? [...new Set(requested)] : officialPluginIds;
const crateName = id => officialPlugins.find(([candidate]) => candidate === id)[1];
run('cargo', ['build', '--locked', '--release', ...plugins.flatMap(id => ['-p', crateName(id)]), '--target', 'wasm32-unknown-unknown']);
for (const id of plugins) {
const component = path.join(root, `target/${id}.component.wasm`);
run(tools, ['component', 'new', path.join(root, `target/wasm32-unknown-unknown/release/${crateName(id).replaceAll('-', '_')}.wasm`), '-o', component]);
run(tools, ['validate', component]);
const staging = mkdtempSync(path.join(root, `target/${id}-package-`));
// SDK pack creates a new output directory; only these generated files are replaced.
rmdirSync(staging);
let manifestFile=path.join(root,`examples/${id}/plugin.json`);
let frontendInput=null;
if(JSON.parse(readFileSync(manifestFile,'utf8')).frontend) {
  const customBuild = path.join(root,`examples/${id}/ui/build.mjs`);
  run(process.execPath,existsSync(customBuild) ? [customBuild] : [path.join(root,'packages/plugin-workbench/build.mjs'),id]);
  frontendInput=mkdtempSync(path.join(root,`target/${id}-frontend-input-`));rmdirSync(frontendInput);
  manifestFile=prepareFrontendInput(manifestFile,path.join(root,`examples/${id}/ui/dist`),frontendInput);
}
try { run('cargo', ['run', '--locked', '-p', 'cobble-plugin-sdk', '--', 'pack', component, manifestFile, staging]); }
finally {if(frontendInput)rmSync(frontendInput,{recursive:true,force:true});}
const output = path.join(root, `target/${id}-package`);
replaceGeneratedPackage(staging, output);
const manifest = JSON.parse(readFileSync(path.join(output, 'plugin.json'), 'utf8'));
if (!existsSync(component) || manifest.id !== id) throw new Error('Invalid generated plugin package.');
console.log(`Built ${manifest.id} ${manifest.version}`);
}
for (const [name, source] of [['viewer', 'contracts/plugin.wit'], ['report', 'contracts/report/report.wit'], ['provider', 'contracts/provider/provider.wit'], ['composer', 'contracts/message-composer/composer.wit'], ['image-layout', 'contracts/image-layout/layout.wit']]) {
  const output = `target/${name}-interface.wasm`;
  run(tools, ['component', 'wit', source, '--wasm', '-o', output]);
  run(tools, ['validate', output]);
}

}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
