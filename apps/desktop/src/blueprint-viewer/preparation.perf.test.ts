// SPDX-License-Identifier: MIT
import { Color, InstancedMesh, BoxGeometry, MeshLambertMaterial, Matrix4 } from 'three';
import { expect, it } from 'vitest';
import { visitBatches } from './batches';

it('records a bounded synthetic preparation baseline without a GPU or a game', async () => {
  const count = 131072, palette = Array.from({ length: 64 }, (_, index) => `minecraft:synthetic_block_${index}[facing=north]`);
  const blocks = Array.from({ length: count }, (_, index) => ({ x: index % 128, y: index % 64, z: index % 256, palette: index % palette.length }));
  const geometry = new BoxGeometry(), material = new MeshLambertMaterial(), matrix = new Matrix4();
  const colorFor = (state: string, target = new Color()) => {
    let hash = 0; for (let index = 0; index < state.length; index++) hash = (hash * 31 + state.charCodeAt(index)) | 0;
    return target.setHSL((hash >>> 0) % 360 / 360, .25, .58);
  };
  const oldMesh = new InstancedMesh(geometry, material, count), newMesh = new InstancedMesh(geometry, material, count);
  // Warm the matrix/color storage before recording CPU preparation only.
  oldMesh.setColorAt(0, new Color()); newMesh.setColorAt(0, new Color());
  const oldColor = new Color(), before = performance.now();
  blocks.forEach((block, index) => { matrix.makeTranslation(block.x, block.y, block.z); oldMesh.setMatrixAt(index, matrix); oldMesh.setColorAt(index, colorFor(palette[block.palette]!, oldColor)); });
  const oldMs = performance.now() - before;
  const colors = palette.map(state => colorFor(state)); let yields = 0, longestBatchMs = 0, batchStart = performance.now();
  const start = batchStart;
  await visitBatches(blocks, (block, index) => { matrix.makeTranslation(block.x, block.y, block.z); newMesh.setMatrixAt(index, matrix); newMesh.setColorAt(index, colors[block.palette]!); }, new AbortController().signal, async () => {
    longestBatchMs = Math.max(longestBatchMs, performance.now() - batchStart); yields++; batchStart = performance.now();
  });
  const newMs = performance.now() - start;
  longestBatchMs = Math.max(longestBatchMs, performance.now() - batchStart);
  expect(newMesh.instanceMatrix.array.every((value, index) => value === oldMesh.instanceMatrix.array[index])).toBe(true);
  expect(newMesh.instanceColor!.array.every((value, index) => value === oldMesh.instanceColor!.array[index])).toBe(true);
  console.info(JSON.stringify({ blocks: count, palette: palette.length, oldMs: +oldMs.toFixed(2), cachedBatchedMs: +newMs.toFixed(2), longestBatchMs: +longestBatchMs.toFixed(2), yields, idleRenderCallsPerSecond: 0, note: 'CPU synthetic; excludes IPC, scheduler delay, GPU upload and draw' }));
  oldMesh.dispose(); newMesh.dispose(); geometry.dispose(); material.dispose();
});
