<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useId } from 'vue';

const props = defineProps<{
  progress: number | null;
  state: 'working' | 'waiting' | 'failed' | 'complete' | 'cancelled';
  sourceLabel: string;
  targetLabel: string;
}>();
type Material = 'rock' | 'grass' | 'water' | 'sand' | 'wood' | 'leaves';
type Block = { x: number; y: number; z: number; w: number; h: number; d: number; material: Material; order: number; top: string; left: string; right: string };
const project = (x: number, y: number, z: number) => `${(x - z) * 22},${(x + z) * 11 - y * 22}`;
function block(x: number, y: number, z: number, w: number, h: number, d: number, material: Material, order: number): Block {
  return { x, y, z, w, h, d, material, order,
    top: [project(x, y + h, z), project(x + w, y + h, z), project(x + w, y + h, z + d), project(x, y + h, z + d)].join(' '),
    left: [project(x, y, z + d), project(x + w, y, z + d), project(x + w, y + h, z + d), project(x, y + h, z + d)].join(' '),
    right: [project(x + w, y, z), project(x + w, y, z + d), project(x + w, y + h, z + d), project(x + w, y + h, z)].join(' '),
  };
}
const foundations: Block[] = [], pieces: Block[] = [];
for (let x = -3; x < 3; x++) for (let z = -3; z < 3; z++) {
  if (Math.abs(x + .5) + Math.abs(z + .5) > 4.2) continue;
  const depth = 1.6 + ((x * x + z * z) % 3) * .22;
  foundations.push(block(x, -depth, z, 1, depth, 1, 'rock', foundations.length));
  const water = x === 0;
  pieces.push(block(x, 0, z, 1, water ? .17 : .32, 1, water ? 'water' : z === -1 && x < 0 ? 'sand' : 'grass', pieces.length));
}
// Construction order is bottom-up; excavation removes the highest pieces first.
const add = (x: number, y: number, z: number, w: number, h: number, d: number, material: Material) => pieces.push(block(x, y, z, w, h, d, material, pieces.length));
add(-1.7, .32, -1.75, .45, 1.8, .45, 'rock');
add(.15, .32, -1.75, .45, 1.8, .45, 'rock');
add(-1.7, 2.12, -1.75, 2.3, .45, .45, 'rock');
for (const [x, z, height] of [[1.4, 1.4, 1.6], [-2.05, .7, 1.25]] as const) {
  add(x, .32, z, .25, height, .25, 'wood');
  add(x - .48, height + .1, z - .48, 1.2, .8, 1.2, 'leaves');
  add(x - .23, height + .9, z - .23, .7, .4, .7, 'leaves');
}
const constructionRank = (piece: Block) => piece.material === 'wood' || piece.material === 'leaves' ? (piece.x < 0 ? 1 : 3) : piece.material === 'rock' ? 4 : piece.x < 0 ? 0 : 2;
[...pieces].sort((a, b) => constructionRank(a) - constructionRank(b) || a.y - b.y).forEach((piece, index) => { piece.order = index; });
const painter = (a: Block, b: Block) => (a.x + a.z) - (b.x + b.z) || a.y - b.y;
foundations.sort(painter);
const painted = [...pieces].sort(painter);
const amount = computed(() => props.state === 'complete' ? 1 : props.progress === null || !Number.isFinite(props.progress) ? null : Math.max(0, Math.min(1, props.progress)));
const built = computed(() => Math.floor((amount.value ?? 0) * pieces.length));
const source = computed(() => painted.filter(piece => piece.order < pieces.length - built.value));
const target = computed(() => painted.filter(piece => piece.order < built.value));
const host = ref<HTMLElement | null>(null), visible = ref(true), pageVisible = ref(true), reduced = ref(false);
const animating = computed(() => props.state === 'working' && amount.value !== 1 && visible.value && pageVisible.value && !reduced.value);
const markerId = `transfer-${useId()}`;
let observer: IntersectionObserver | undefined, media: MediaQueryList | undefined;
const syncPage = () => { pageVisible.value = !document.hidden; };
const syncMotion = () => { reduced.value = media?.matches ?? false; };
onMounted(() => {
  syncPage();
  document.addEventListener('visibilitychange', syncPage);
  media = window.matchMedia?.('(prefers-reduced-motion: reduce)');
  syncMotion(); media?.addEventListener('change', syncMotion);
  if (typeof IntersectionObserver !== 'undefined') {
    observer = new IntersectionObserver(([entry]) => { if (entry) visible.value = entry.isIntersecting; });
    if (host.value) observer.observe(host.value);
  }
});
onBeforeUnmount(() => {
  observer?.disconnect(); document.removeEventListener('visibilitychange', syncPage); media?.removeEventListener('change', syncMotion);
});
</script>

<template>
  <figure
    ref="host"
    class="transfer-world-scene"
    :data-state="state"
    :data-animating="animating"
    :data-progress-known="amount !== null"
    :aria-label="`${sourceLabel} → ${targetLabel}`"
  >
    <svg
      class="transfer-world-art"
      viewBox="0 0 900 330"
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient :id="markerId">
          <stop
            stop-color="currentColor"
            stop-opacity="0"
          />
          <stop
            offset=".3"
            stop-color="currentColor"
            stop-opacity=".6"
          />
          <stop
            offset="1"
            stop-color="currentColor"
            stop-opacity=".15"
          />
        </linearGradient>
      </defs>
      <ellipse
        class="transfer-island-shadow"
        cx="230"
        cy="289"
        rx="117"
        ry="21"
      />
      <ellipse
        class="transfer-island-shadow"
        cx="670"
        cy="289"
        rx="117"
        ry="21"
      />
      <path
        class="transfer-arc"
        d="M 240 163 C 350 44 550 44 660 163"
        :stroke="`url(#${markerId})`"
      />
      <g
        v-for="[name, center, blocks] in [['source', 230, source], ['target', 670, target]] as const"
        :key="name"
        :transform="`translate(${center} 200)`"
        :data-world="name"
        :data-built-blocks="blocks.length"
      >
        <g
          v-for="base in foundations"
          :key="`base-${base.order}`"
          class="world-block material-rock foundation-block"
        >
          <polygon
            class="block-left"
            :points="base.left"
          />
          <polygon
            class="block-right"
            :points="base.right"
          />
          <polygon
            class="block-top"
            :points="base.top"
          />
        </g>
        <g
          v-for="piece in blocks"
          :key="piece.order"
          class="world-block build-block"
          :class="`material-${piece.material}`"
        >
          <polygon
            class="block-left"
            :points="piece.left"
          />
          <polygon
            class="block-right"
            :points="piece.right"
          />
          <polygon
            class="block-top"
            :points="piece.top"
          />
        </g>
      </g>
      <g
        v-if="state === 'working' && amount !== 1"
        class="transfer-particles"
        transform="translate(240 163)"
      >
        <g
          v-for="index in 4"
          :key="index"
          class="flying-block"
          :style="{ animationDelay: `${index * -.9}s` }"
        >
          <path
            class="flying-left"
            d="m-9 0 9 5v11l-9-5Z"
          />
          <path
            class="flying-right"
            d="m0 5 9-5v11l-9 5Z"
          />
          <path
            class="flying-top"
            d="m-9 0 9-5 9 5-9 5Z"
          />
        </g>
      </g>
    </svg>
    <figcaption class="transfer-world-labels">
      <span :title="sourceLabel">{{ sourceLabel }}</span>
      <span :title="targetLabel">{{ targetLabel }}</span>
    </figcaption>
  </figure>
</template>

<style scoped>
.transfer-world-scene{--rock-top:color-mix(in srgb,var(--ui-surface-3,#e7ebef) 48%,#aab49c);--rock-left:color-mix(in srgb,var(--ui-surface-3,#e7ebef) 25%,#818d78);--rock-right:color-mix(in srgb,var(--ui-surface-3,#e7ebef) 35%,#939f87);width:min(100%,900px);margin:0 auto;color:var(--ui-focus,#466c20);isolation:isolate}
.transfer-world-art{display:block;width:100%;height:auto;overflow:visible;max-height:340px;min-height:150px;stroke:none}
.transfer-island-shadow{fill:color-mix(in srgb,var(--ui-text,#242a30) 6%,transparent)}
.world-block{--top:var(--rock-top);--left:var(--rock-left);--right:var(--rock-right)}
.block-top{fill:var(--top)}.block-left{fill:var(--left)}.block-right{fill:var(--right)}
.material-grass{--top:#acc885;--left:#879c6e;--right:#94ad76}.material-water{--top:#9bbfc1;--left:#6d9fa4;--right:#83b0b4}.material-sand{--top:#ddcfaa;--left:#afa080;--right:#c3b48f}.material-wood{--top:#b8a187;--left:#8b7762;--right:#9e8a71}.material-leaves{--top:#9ebb78;--left:#678653;--right:#7e9b60}
.foundation-block .block-top{stroke:color-mix(in srgb,var(--ui-surface-1,#fff) 14%,transparent);stroke-width:.45;stroke-linejoin:round}
.transfer-arc{fill:none;stroke-width:1.8;stroke-dasharray:3 8;stroke-linecap:round}
.transfer-world-scene[data-state=failed]{color:var(--ui-danger,#a02b28)}
.transfer-world-scene[data-state=cancelled]{color:var(--ui-text-muted,#5c6871)}
.transfer-world-scene[data-state=complete]{color:var(--ui-success,#34601d)}
.flying-block{animation:world-flight 3.6s cubic-bezier(.35,.12,.58,.92) infinite;animation-play-state:paused;transform-origin:0 0;opacity:0}
[data-animating=true] .flying-block{animation-play-state:running}
.flying-top{fill:#c7dc9b}.flying-left{fill:#839966}.flying-right{fill:#a2bb7f}
.transfer-world-labels{display:grid;grid-template-columns:1fr 1fr;gap:16%;padding:0 9%;margin-top:-5px;color:var(--ui-text-muted,#5c6871);font-size:var(--ui-text-label,14px);line-height:var(--ui-leading-label,1.5);text-align:center}
.transfer-world-labels span{min-width:0;overflow-wrap:anywhere}
@keyframes world-flight{0%,6%{opacity:0;transform:translate(0,0) scale(.65) rotate(-8deg)}14%{opacity:1}48%{opacity:1;transform:translate(205px,-88px) scale(1) rotate(12deg)}82%{opacity:1;transform:translate(410px,-3px) scale(.8) rotate(-6deg)}92%,100%{opacity:0;transform:translate(420px,0) scale(.55) rotate(0)}}
@media(max-width:560px){.transfer-world-art{min-height:145px}.transfer-world-labels{font-size:var(--ui-text-caption,12px);gap:12%;padding:0 6%;margin-top:0}}
@media(prefers-reduced-motion:reduce){.flying-block{animation:none;display:none}}
[data-motion="reduced"] .flying-block{animation:none;display:none}
</style>
