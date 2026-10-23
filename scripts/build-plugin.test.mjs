// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { replaceGeneratedPackage, prepareFrontendInput } from './build-plugin.mjs';

test('replaces declared artwork and removes only a previous declared icon', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'cobble-package-assets-'));
  const output = path.join(root, 'output'); mkdirSync(output);
  const stage = (name, icon) => {
    const directory = path.join(root, name); mkdirSync(directory);
    writeFileSync(path.join(directory, 'plugin.json'), JSON.stringify({ id: 'fixture', ...(icon ? { icon: { file: 'icon.png' } } : {}) }));
    writeFileSync(path.join(directory, 'component.wasm'), 'component');
    if (icon) writeFileSync(path.join(directory, 'icon.png'), icon);
    return directory;
  };
  try {
    writeFileSync(path.join(output, 'user-file.txt'), 'keep');
    replaceGeneratedPackage(stage('first', 'original'), output);
    assert.equal(readFileSync(path.join(output, 'icon.png'), 'utf8'), 'original');
    replaceGeneratedPackage(stage('second', 'updated'), output);
    assert.equal(readFileSync(path.join(output, 'icon.png'), 'utf8'), 'updated');
    replaceGeneratedPackage(stage('third'), output);
    assert.equal(existsSync(path.join(output, 'icon.png')), false);
    assert.equal(readFileSync(path.join(output, 'user-file.txt'), 'utf8'), 'keep');
    assert.equal(existsSync(path.join(root, 'second')), false);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('indexes and replaces only bounded declared frontend assets', () => {
  const root=mkdtempSync(path.join(os.tmpdir(),'cobble-frontend-build-'));
  try {
    const manifestFile=path.join(root,'source.json');
    writeFileSync(manifestFile,JSON.stringify({id:'original-fixture',frontend:{api_version:'0.1.0',entry:'ui/index.html',assets_sha256:'0'.repeat(64)}}));
    const dist=path.join(root,'dist');mkdirSync(path.join(dist,'assets'),{recursive:true});writeFileSync(path.join(dist,'index.html'),'<!doctype html><script src="./assets/app.js"></script>');writeFileSync(path.join(dist,'assets/app.js'),'export const original=true;');
    const input=path.join(root,'input');prepareFrontendInput(manifestFile,dist,input);writeFileSync(path.join(input,'component.wasm'),'original-component-fixture');
    const manifest=JSON.parse(readFileSync(path.join(input,'plugin.json')));assert.notEqual(manifest.frontend.assets_sha256,'0'.repeat(64));
    const index=JSON.parse(readFileSync(path.join(input,'frontend-assets.json')));assert.equal(index.files.length,2);assert.equal(index.files.find(file=>file.path==='ui/index.html').bytes,readFileSync(path.join(dist,'index.html')).length);
    const output=path.join(root,'output');replaceGeneratedPackage(input,output);assert.equal(readFileSync(path.join(output,'ui/assets/app.js'),'utf8'),'export const original=true;');
    const bad=path.join(root,'bad');prepareFrontendInput(manifestFile,dist,bad);writeFileSync(path.join(bad,'component.wasm'),'original-component-fixture');writeFileSync(path.join(bad,'ui/assets/app.js'),'tampered');assert.throws(()=>replaceGeneratedPackage(bad,output),/digest mismatch/);assert.equal(readFileSync(path.join(output,'ui/assets/app.js'),'utf8'),'export const original=true;');
    writeFileSync(path.join(dist,'assets/unsupported.svg'),'<svg/>');assert.throws(()=>prepareFrontendInput(manifestFile,dist,path.join(root,'unsupported')),/Invalid frontend build output/);
  } finally {rmSync(root,{recursive:true,force:true});}
});
