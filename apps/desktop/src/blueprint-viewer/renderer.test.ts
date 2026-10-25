// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import * as THREE from 'three';
import { createBlueprintRenderer } from './renderer';

const fake = vi.hoisted(() => ({ render: vi.fn(), setSize: vi.fn(), setPixelRatio: vi.fn(), disconnect: vi.fn(), dispose: vi.fn(), controlsDispose: vi.fn(), resize: undefined as (() => void) | undefined, change: undefined as (() => void) | undefined }));
vi.mock('three', async importOriginal => ({
  ...await importOriginal<typeof import('three')>(),
  WebGLRenderer: class {
    capabilities = { maxTextures: 16, maxAttributes: 16, maxTextureSize: 4096 };
    setPixelRatio = fake.setPixelRatio; setSize = fake.setSize; render = fake.render; dispose = fake.dispose;
  },
}));
vi.mock('three/examples/jsm/controls/OrbitControls.js', () => ({ OrbitControls: class {
  addEventListener(_type: string, callback: () => void) { fake.change = callback; }
  removeEventListener() { fake.change = undefined; }
  dispose = fake.controlsDispose;
} }));
const blueprint = { resourceId: 'test', name: 'Test', scene: { width: 32, height: 32, length: 32, palette: ['minecraft:stone'], materials: [], chunk_count: 1 } };
let bounds = { width: 400, height: 320 };
beforeEach(() => {
  bounds = { width: 400, height: 320 };
  vi.spyOn(HTMLCanvasElement.prototype, 'getBoundingClientRect').mockImplementation(() => bounds as DOMRect);
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { fake.resize = callback; }
    observe() {} disconnect = fake.disconnect;
  });
});
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.clearAllMocks(); vi.unstubAllGlobals(); });

it('renders only on scene or control changes and stops rendering when closed', async () => {
  vi.useFakeTimers();
  const controller = new AbortController();
  const viewer = await createBlueprintRenderer(document.createElement('canvas'), blueprint, controller.signal);
  expect(viewer).toBeDefined();
  expect(fake.render).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(1000);
  expect(fake.render).toHaveBeenCalledTimes(1);
  fake.change?.(); expect(fake.render).toHaveBeenCalledTimes(2);
  controller.abort();
  expect(fake.dispose).toHaveBeenCalledOnce(); expect(fake.controlsDispose).toHaveBeenCalledOnce();
  expect(await viewer!.addChunk([{ x: 0, y: 0, z: 0, palette_index: 0 }])).toBe(false);
  expect(fake.render).toHaveBeenCalledTimes(2);
  expect(vi.getTimerCount()).toBe(0);
});

it('tracks actual canvas size and display density without churning hidden or unchanged buffers', async () => {
  const controller = new AbortController();
  await createBlueprintRenderer(document.createElement('canvas'), blueprint, controller.signal);
  expect(fake.setSize).toHaveBeenLastCalledWith(400, 320, false);
  bounds = { width: 200, height: 320 }; fake.resize?.();
  const camera = fake.render.mock.lastCall![1] as THREE.PerspectiveCamera;
  expect(camera.aspect).toBe(.625);
  expect(fake.setSize).toHaveBeenLastCalledWith(200, 320, false);
  const calls = fake.setSize.mock.calls.length;
  fake.resize?.(); expect(fake.setSize).toHaveBeenCalledTimes(calls);
  bounds = { width: 0, height: 0 }; fake.resize?.();
  expect(fake.setSize).toHaveBeenCalledTimes(calls);
  const hiddenRenderCalls = fake.render.mock.calls.length;
  fake.change?.(); expect(fake.render).toHaveBeenCalledTimes(hiddenRenderCalls);
  bounds = { width: 200, height: 320 }; fake.resize?.();
  expect(fake.setSize).toHaveBeenCalledTimes(calls);
  expect(fake.render).toHaveBeenCalledTimes(hiddenRenderCalls + 1);
  bounds = { width: 640, height: 320 };
  vi.stubGlobal('devicePixelRatio', 3); window.dispatchEvent(new Event('resize'));
  expect(fake.setPixelRatio).toHaveBeenLastCalledWith(2);
  expect(camera.aspect).toBe(2);
  controller.abort(); expect(fake.disconnect).toHaveBeenCalledOnce();
  const renderCalls = fake.render.mock.calls.length;
  bounds = { width: 800, height: 320 }; fake.resize?.(); window.dispatchEvent(new Event('resize'));
  expect(fake.render).toHaveBeenCalledTimes(renderCalls);
});

it('defers viewport allocation and rendering until a hidden canvas has a usable size', async () => {
  bounds = { width: 0, height: 0 };
  const controller = new AbortController();
  await createBlueprintRenderer(document.createElement('canvas'), blueprint, controller.signal);
  expect(fake.setSize).not.toHaveBeenCalled(); expect(fake.render).not.toHaveBeenCalled();
  bounds = { width: 300, height: 320 }; fake.resize?.();
  expect(fake.setSize).toHaveBeenCalledOnce(); expect(fake.render).toHaveBeenCalledOnce();
  controller.abort();
});

it('releases partially prepared GPU resources and resolves pending preparation on close', async () => {
  vi.useFakeTimers();
  const meshDispose = vi.spyOn(THREE.InstancedMesh.prototype, 'dispose');
  const geometryDispose = vi.spyOn(THREE.BoxGeometry.prototype, 'dispose');
  const materialDispose = vi.spyOn(THREE.MeshLambertMaterial.prototype, 'dispose');
  const controller = new AbortController();
  const viewer = await createBlueprintRenderer(document.createElement('canvas'), blueprint, controller.signal);
  const pending = viewer!.addChunk(Array.from({ length: 10000 }, (_, x) => ({ x, y: 0, z: 0, palette_index: 0 })));
  controller.abort();
  expect(await pending).toBe(false);
  expect(meshDispose).toHaveBeenCalledOnce(); expect(geometryDispose).toHaveBeenCalledTimes(2); expect(materialDispose).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
  expect(fake.render).toHaveBeenCalledTimes(1);
});
