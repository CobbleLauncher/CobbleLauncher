// SPDX-License-Identifier: MIT
// Bake selected installed Lucide icons into one static sprite. No Vue ships to the site.
import { createRequire } from 'node:module';
import { writeFile, readFile } from 'node:fs/promises';
const require=createRequire(new URL('../desktop/package.json',import.meta.url));
const {createSSRApp,h}=require('vue');
const {renderToString}=require('vue/server-renderer');
const icons=require('@lucide/vue');
const names=['ArrowUpRight','ArrowDown','ArrowDownRight','SunMoon','RotateCw','Coffee','Download','Undo2','Shapes','Compass','ScanLine','HardDrive','Leaf','Code2','X','Play','Check','Zap','Mountain','Layers','Blocks','History','GitCompareArrows'];
const symbols=[];
for(const name of names){
  const svg=await renderToString(createSSRApp({render:()=>h(icons[name])}));
  const id=name.replace(/([a-z0-9])([A-Z])/g,'$1-$2').replace(/([a-z])(\d)/g,'$1-$2').toLowerCase();
  symbols.push(`<symbol id="${id}" viewBox="0 0 24 24">${svg.replace(/^.*?<svg[^>]*>/,'').replace(/<\/svg>.*$/,'')}</symbol>`);
}
await writeFile(new URL('./dist/icons.svg',import.meta.url),`<svg xmlns="http://www.w3.org/2000/svg">${symbols.join('\n')}</svg>`);
const license=await readFile(require.resolve('@lucide/vue/package.json').replace(/package.json$/,'LICENSE'),'utf8');
const projectLicense=await readFile(new URL('../../LICENSE',import.meta.url),'utf8');
await writeFile(new URL('./dist/licenses.txt',import.meta.url),`Cobble landing page:\n${projectLicense}\nOriginal procedural terrain and geometric illustrations by Cobble contributors.\nNo Minecraft art or textures are distributed with this page.\n\nSelected Lucide icons, @lucide/vue 1.61.0:\n${license}`);
console.log(`Generated ${names.length} Lucide symbols with upstream license.`);
