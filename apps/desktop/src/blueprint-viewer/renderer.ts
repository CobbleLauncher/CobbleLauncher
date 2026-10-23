// SPDX-License-Identifier: MIT
import type { OpenedBlueprint, SceneBlock } from '../blueprints';
import { visitBatches } from './batches';
import {textureAtlas,textureVertexHeader,textureVertex,textureFragmentHeader,textureFragment,type ViewerTextures} from './textures';

export async function createBlueprintRenderer(canvas: HTMLCanvasElement, value: OpenedBlueprint, signal: AbortSignal) {
  const [THREE, { OrbitControls }] = await Promise.all([import('three'), import('three/examples/jsm/controls/OrbitControls.js')]);
  if (signal.aborted) return;
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(45, 1, .1, 10000);
  const size = Math.max(value.scene.width, value.scene.height, value.scene.length, 1);
  camera.position.set(size * 1.6, size * 1.3, size * 1.6); camera.lookAt(0, 0, 0);
  const controls = new OrbitControls(camera, canvas);
  let width = 0, height = 0, pixelRatio = 0, visible = false;
  const render = () => { if (!signal.aborted && visible) renderer.render(scene, camera); };
  const resize = () => {
    if (signal.aborted) return;
    const bounds = canvas.getBoundingClientRect();
    const nextWidth = Math.round(bounds.width), nextHeight = Math.round(bounds.height);
    const wasVisible = visible;
    visible = nextWidth > 0 && nextHeight > 0;
    if (!visible) return;
    const nextRatio = Math.min(window.devicePixelRatio || 1, 2);
    if (width === nextWidth && height === nextHeight && pixelRatio === nextRatio) { if (!wasVisible) render(); return; }
    if (pixelRatio !== nextRatio) renderer.setPixelRatio(nextRatio);
    width = nextWidth; height = nextHeight; pixelRatio = nextRatio;
    renderer.setSize(width, height, false);
    camera.aspect = width / height; camera.updateProjectionMatrix();
    render();
  };
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  window.addEventListener('resize', resize);
  controls.addEventListener('change', render);
  scene.add(new THREE.AmbientLight(0xffffff, 1));
  const light = new THREE.DirectionalLight(0xffffff, 1.5); light.position.set(3, 5, 2); scene.add(light);
  const geometry = new THREE.BoxGeometry(.96, .96, .96), material = new THREE.MeshLambertMaterial();
  const normal=geometry.getAttribute('normal');const faces=new Float32Array(normal.count);
  for(let index=0;index<normal.count;index++)faces[index]=normal.getX(index)>0?0:normal.getX(index)<0?1:normal.getY(index)>0?2:normal.getY(index)<0?3:normal.getZ(index)>0?4:5;
  geometry.setAttribute('faceId',new THREE.BufferAttribute(faces,1));
  const empty=new THREE.DataTexture(new Uint8Array([0,0,0,0]),1,1);empty.needsUpdate=true;
  const uniforms={blueprintAtlas:{value:empty as InstanceType<typeof THREE.Texture>},blueprintFaces:{value:empty as InstanceType<typeof THREE.Texture>},blueprintPaletteCount:{value:1},blueprintAtlasSide:{value:1},blueprintTexturesEnabled:{value:0}};
  const canTexture=renderer.capabilities.maxTextures>=2&&renderer.capabilities.maxAttributes>=10;
  if(canTexture){material.onBeforeCompile=shader=>{Object.assign(shader.uniforms,uniforms);shader.vertexShader=textureVertexHeader+shader.vertexShader.replace('#include <uv_vertex>',textureVertex);shader.fragmentShader=textureFragmentHeader+shader.fragmentShader.replace('#include <color_fragment>',textureFragment);};material.customProgramCacheKey=()=> 'blueprint-texture-atlas-v1';}
  let textureGeneration=0;let atlas:InstanceType<typeof THREE.Texture>|undefined,faceMap:InstanceType<typeof THREE.Texture>|undefined;
  const meshes: InstanceType<typeof THREE.InstancedMesh>[] = [];
  // Palette work scales with distinct states instead of blocks.
  const colors = value.scene.palette.map(state => {
    let hash = 0; for (let index = 0; index < state.length; index++) hash = (hash * 31 + state.charCodeAt(index)) | 0;
    return new THREE.Color().setHSL((hash >>> 0) % 360 / 360, .25, .58);
  });
  const fallback = new THREE.Color().setHSL(0, .25, .58), matrix = new THREE.Matrix4();
  let pendingTimer: ReturnType<typeof setTimeout> | undefined, resume: (() => void) | undefined;
  const yieldControl = () => new Promise<void>(resolve => {
    resume = resolve; pendingTimer = setTimeout(() => { pendingTimer = undefined; resume = undefined; resolve(); }, 0);
  });
  const dispose = () => {
    if (pendingTimer !== undefined) clearTimeout(pendingTimer);
    resume?.(); resume = undefined;
    observer.disconnect(); window.removeEventListener('resize', resize);
    controls.removeEventListener('change', render); controls.dispose();
    textureGeneration++;
    for (const mesh of meshes) { scene.remove(mesh); mesh.geometry.dispose(); mesh.dispose(); }
    atlas?.dispose();faceMap?.dispose();empty.dispose();geometry.dispose(); material.dispose(); renderer.dispose();
  };
  signal.addEventListener('abort', dispose, { once: true });
  resize();
  return {
    snapshot() {
      if (signal.aborted || !visible) return null;
      render();
      const thumbnail = document.createElement('canvas'); thumbnail.width = 320; thumbnail.height = 180;
      const context = thumbnail.getContext('2d'); if (!context) return null;
      const cropWidth = Math.min(canvas.width, canvas.height * 16 / 9);
      const cropHeight = Math.min(canvas.height, canvas.width * 9 / 16);
      context.drawImage(canvas, (canvas.width - cropWidth) / 2, (canvas.height - cropHeight) / 2,
        cropWidth, cropHeight, 0, 0, 320, 180);
      return thumbnail.toDataURL('image/png');
    },
    async setTextures(value:ViewerTextures|null){
      const current=++textureGeneration;uniforms.blueprintTexturesEnabled.value=0;render();
      if(!value||!canTexture||!value.textures.length)return false;
      try{
        const prepared=await textureAtlas(value,signal,renderer.capabilities.maxTextureSize);
        if(signal.aborted||current!==textureGeneration)return false;
        const nextAtlas=new THREE.CanvasTexture(prepared.canvas);nextAtlas.colorSpace=THREE.SRGBColorSpace;nextAtlas.magFilter=nextAtlas.minFilter=THREE.NearestFilter;nextAtlas.generateMipmaps=false;
        const nextFaces=new THREE.DataTexture(prepared.mapping,6,Math.max(1,value.paletteFaces.length));nextFaces.magFilter=nextFaces.minFilter=THREE.NearestFilter;nextFaces.generateMipmaps=false;nextFaces.needsUpdate=true;
        atlas?.dispose();faceMap?.dispose();atlas=nextAtlas;faceMap=nextFaces;
        uniforms.blueprintAtlas.value=nextAtlas;uniforms.blueprintFaces.value=nextFaces;uniforms.blueprintPaletteCount.value=Math.max(1,value.paletteFaces.length);uniforms.blueprintAtlasSide.value=prepared.side;uniforms.blueprintTexturesEnabled.value=1;render();return true;
      }catch{return false;}
    },
    async addChunk(blocks: SceneBlock[]) {
      if (signal.aborted) return false;
      const chunkGeometry=geometry.clone();const palette=new Float32Array(blocks.length);chunkGeometry.setAttribute('paletteIndex',new THREE.InstancedBufferAttribute(palette,1));
      const mesh = new THREE.InstancedMesh(chunkGeometry, material, blocks.length);
      meshes.push(mesh); // Also dispose partially filled meshes when cancelled.
      const complete = await visitBatches(blocks, (block, index) => {
        matrix.makeTranslation(block.x - value.scene.width / 2, block.y - value.scene.height / 2, block.z - value.scene.length / 2);
        mesh.setMatrixAt(index, matrix); mesh.setColorAt(index, colors[block.palette_index] ?? fallback);
        palette[index]=block.palette_index;
      }, signal, yieldControl);
      if (!complete) return false;
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
      scene.add(mesh); render();
      await yieldControl();
      return !signal.aborted;
    },
  };
}
