<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import type { FirstRunStep } from '../first-run';

defineProps<{ step: Exclude<FirstRunStep, 'consent'> }>();
</script>

<template>
  <figure
    class="guide-scene"
    aria-hidden="true"
  >
    <svg
      viewBox="0 0 480 280"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <rect
        class="scene-backdrop"
        x="8"
        y="8"
        width="464"
        height="264"
        rx="28"
      />
      <path
        class="scene-orbit"
        d="M60 212C88 72 342 28 423 136"
      />
      <circle
        class="scene-spark"
        cx="88"
        cy="66"
        r="4"
      />
      <circle
        class="scene-spark scene-spark-small"
        cx="405"
        cy="210"
        r="3"
      />

      <g
        v-if="step === 'welcome'"
        class="scene-window"
      >
        <rect
          class="scene-panel"
          x="93"
          y="49"
          width="294"
          height="184"
          rx="18"
        />
        <path
          class="scene-line"
          d="M94 84H386"
        />
        <circle
          class="scene-dot"
          cx="115"
          cy="67"
          r="4"
        />
        <circle
          class="scene-dot"
          cx="130"
          cy="67"
          r="4"
        />
        <circle
          class="scene-dot"
          cx="145"
          cy="67"
          r="4"
        />
        <rect
          class="scene-rail"
          x="109"
          y="101"
          width="54"
          height="116"
          rx="9"
        />
        <rect
          class="scene-accent"
          x="120"
          y="114"
          width="32"
          height="7"
          rx="3.5"
        />
        <rect
          class="scene-rail-line"
          x="120"
          y="132"
          width="23"
          height="5"
          rx="2.5"
        />
        <rect
          class="scene-rail-line"
          x="120"
          y="146"
          width="30"
          height="5"
          rx="2.5"
        />
        <rect
          class="scene-card"
          x="178"
          y="101"
          width="188"
          height="48"
          rx="10"
        />
        <rect
          class="scene-card"
          x="178"
          y="160"
          width="88"
          height="57"
          rx="10"
        />
        <rect
          class="scene-card"
          x="278"
          y="160"
          width="88"
          height="57"
          rx="10"
        />
        <g class="scene-cube">
          <path d="M195 115 207 108 219 115 207 122 195 115Z" />
          <path d="M195 115V130L207 137V122L195 115Z" />
          <path d="M219 115V130L207 137V122L219 115Z" />
        </g>
        <rect
          class="scene-card-line"
          x="230"
          y="113"
          width="87"
          height="6"
          rx="3"
        />
        <rect
          class="scene-card-line scene-card-line-short"
          x="230"
          y="126"
          width="55"
          height="5"
          rx="2.5"
        />
      </g>

      <g
        v-else-if="step === 'profile'"
        class="scene-window"
      >
        <rect
          class="scene-panel"
          x="75"
          y="43"
          width="330"
          height="194"
          rx="20"
        />
        <circle
          class="scene-avatar-halo"
          cx="190"
          cy="132"
          r="66"
        />
        <circle
          class="scene-avatar"
          cx="190"
          cy="132"
          r="43"
        />
        <circle
          class="scene-avatar-head"
          cx="190"
          cy="119"
          r="13"
        />
        <path
          class="scene-avatar-body"
          d="M165 157C169 140 179 133 190 133C201 133 211 140 215 157"
        />
        <rect
          class="scene-card"
          x="270"
          y="84"
          width="104"
          height="38"
          rx="10"
        />
        <rect
          class="scene-accent"
          x="283"
          y="99"
          width="12"
          height="8"
          rx="4"
        />
        <rect
          class="scene-card-line"
          x="303"
          y="98"
          width="56"
          height="6"
          rx="3"
        />
        <rect
          class="scene-card"
          x="270"
          y="135"
          width="104"
          height="38"
          rx="10"
        />
        <circle
          class="scene-dot"
          cx="290"
          cy="154"
          r="6"
        />
        <rect
          class="scene-card-line"
          x="303"
          y="151"
          width="48"
          height="6"
          rx="3"
        />
        <path
          class="scene-check"
          d="m331 196 7 7 15-16"
        />
      </g>

      <g
        v-else-if="step === 'game'"
        class="scene-window"
      >
        <rect
          class="scene-panel"
          x="89"
          y="44"
          width="302"
          height="192"
          rx="20"
        />
        <rect
          class="scene-card"
          x="109"
          y="66"
          width="262"
          height="130"
          rx="14"
        />
        <path
          class="scene-land"
          d="m126 151 64-42 62 37 50-31 51 35v29H126v-28Z"
        />
        <path
          class="scene-land-back"
          d="m126 151 64 19v9h-64v-28Z"
        />
        <path
          class="scene-land-side"
          d="m190 170 62-24v33h-62v-9Z"
        />
        <path
          class="scene-land-back"
          d="m252 146 50-31v64h-50v-33Z"
        />
        <path
          class="scene-land-side"
          d="m302 115 51 35v29h-51v-64Z"
        />
        <path
          class="scene-cube-top"
          d="m192 82 21-12 21 12-21 12-21-12Z"
        />
        <path
          class="scene-cube-left"
          d="M192 82v25l21 12V94l-21-12Z"
        />
        <path
          class="scene-cube-right"
          d="M234 82v25l-21 12V94l21-12Z"
        />
        <rect
          class="scene-progress-track"
          x="130"
          y="210"
          width="220"
          height="7"
          rx="3.5"
        />
        <rect
          class="scene-progress"
          x="130"
          y="210"
          width="142"
          height="7"
          rx="3.5"
        />
      </g>

      <g
        v-else
        class="scene-window"
      >
        <rect
          class="scene-panel"
          x="103"
          y="55"
          width="274"
          height="170"
          rx="20"
        />
        <path
          class="scene-link"
          d="m180 139 55-35 61 36-61 38-55-39Z"
        />
        <circle
          class="scene-node scene-node-left"
          cx="180"
          cy="139"
          r="24"
        />
        <circle
          class="scene-node scene-node-center"
          cx="235"
          cy="104"
          r="28"
        />
        <circle
          class="scene-node scene-node-right"
          cx="296"
          cy="140"
          r="24"
        />
        <circle
          class="scene-node scene-node-bottom"
          cx="235"
          cy="178"
          r="21"
        />
        <path
          class="scene-cube-top"
          d="m226 99 9-5 9 5-9 5-9-5Z"
        />
        <path
          class="scene-cube-left"
          d="M226 99v11l9 5v-11l-9-5Z"
        />
        <path
          class="scene-cube-right"
          d="M244 99v11l-9 5v-11l9-5Z"
        />
        <path
          class="scene-signal"
          d="M145 102c-9 9-9 24 0 33M325 102c9 9 9 24 0 33"
        />
      </g>
    </svg>
  </figure>
</template>

<style scoped>
.guide-scene{width:min(100%,440px);margin:0 auto;animation:guide-scene-arrive .38s cubic-bezier(.2,.7,.25,1) both}
.guide-scene svg{display:block;width:100%;height:auto;overflow:visible}
.scene-backdrop{fill:var(--ui-surface-2);stroke:var(--ui-border);stroke-width:1.5}
.scene-orbit{stroke:var(--ui-border-strong);stroke-width:1.5;stroke-dasharray:3 8;opacity:.65}
.scene-spark{fill:var(--ui-focus);opacity:.65}
.scene-spark-small{opacity:.4}
.scene-panel{fill:var(--ui-surface-1);stroke:var(--ui-border-strong);stroke-width:1.5}
.scene-line{stroke:var(--ui-border);stroke-width:1.5}
.scene-dot{fill:var(--ui-focus);opacity:.6}
.scene-rail{fill:var(--ui-surface-2)}
.scene-accent{fill:var(--ui-focus)}
.scene-rail-line,.scene-card-line{fill:var(--ui-text-muted);opacity:.42}
.scene-card{fill:var(--ui-surface-2);stroke:var(--ui-border);stroke-width:1}
.scene-card-line-short{opacity:.25}
.scene-cube path{fill:var(--ui-focus)}
.scene-cube path:nth-child(2),.scene-cube-left{opacity:.72}
.scene-cube path:nth-child(3),.scene-cube-right{opacity:.9}
.scene-avatar-halo{fill:color-mix(in srgb,var(--ui-focus) 12%,var(--ui-surface-1))}
.scene-avatar{fill:color-mix(in srgb,var(--ui-focus) 18%,var(--ui-surface-1));stroke:var(--ui-focus);stroke-width:1.5}
.scene-avatar-head{fill:var(--ui-focus);opacity:.85}
.scene-avatar-body{stroke:var(--ui-focus);stroke-width:9;stroke-linecap:round;opacity:.75}
.scene-check{stroke:var(--ui-focus);stroke-width:4;stroke-linecap:round;stroke-linejoin:round}
.scene-land{fill:var(--ui-focus);opacity:.82}
.scene-land-back{fill:var(--ui-focus);opacity:.58}
.scene-land-side{fill:var(--ui-focus);opacity:.7}
.scene-cube-top{fill:var(--ui-focus)}
.scene-progress-track{fill:var(--ui-surface-3)}
.scene-progress{fill:var(--ui-focus)}
.scene-link{stroke:var(--ui-border-strong);stroke-width:3;stroke-dasharray:5 6}
.scene-node{fill:var(--ui-surface-2);stroke:var(--ui-border-strong);stroke-width:2}
.scene-node-center{fill:color-mix(in srgb,var(--ui-focus) 16%,var(--ui-surface-1));stroke:var(--ui-focus)}
.scene-node-left,.scene-node-bottom{stroke:var(--ui-focus);opacity:.8}
.scene-signal{stroke:var(--ui-focus);stroke-width:2;stroke-linecap:round;opacity:.65}
@keyframes guide-scene-arrive{from{opacity:0;transform:translateY(8px) scale(.985)}to{opacity:1;transform:translateY(0) scale(1)}}
@media(prefers-reduced-motion:reduce){.guide-scene{animation:none}}
:global(:root[data-motion="reduced"]) .guide-scene{animation:none}
</style>
