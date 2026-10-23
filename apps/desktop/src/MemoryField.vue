<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed} from 'vue';
import {SliderRoot,SliderTrack,SliderRange,SliderThumb} from 'reka-ui';
import {t} from './i18n';
const props=defineProps<{modelValue:number;label:string;id:string;disabled:boolean}>();
const emit=defineEmits<{'update:modelValue':[value:number]}>();
// Allocation stops keep common Minecraft values easy to select without assuming installed RAM.
const amounts=[512,1024,2048,3072,4096,6144,8192,12288,16384,24576,32768,49152,65536,98304,131072,196608,262144];
const position=computed(()=>amounts.reduce((closest,amount,index)=>Math.abs(amount-props.modelValue)<Math.abs(amounts[closest]!-props.modelValue)?index:closest,0));
function select(values:number[]|undefined){if(props.disabled||values?.[0]===undefined)return;const value=amounts[Math.round(values[0])];if(value!==undefined)emit('update:modelValue',value);}
function type(event:Event){if(!props.disabled)emit('update:modelValue',Number((event.target as HTMLInputElement).value));}
</script>
<template>
  <div class="memory-field">
    <div class="memory-control">
      <div class="memory-slider-area">
        <SliderRoot
          class="memory-slider"
          :model-value="[position]"
          :min="0"
          :max="amounts.length-1"
          :step="1"
          :disabled="disabled"
          @update:model-value="select"
        >
          <SliderTrack class="memory-slider-track">
            <SliderRange class="memory-slider-range" />
          </SliderTrack>
          <SliderThumb
            class="memory-slider-thumb"
            :aria-label="t('memoryControl.adjust',{label})"
            :aria-valuetext="`${modelValue} MiB`"
          />
        </SliderRoot>
        <div
          class="memory-limits"
          aria-hidden="true"
        >
          <span>512 MiB</span><span>256 GiB</span>
        </div>
      </div>
      <div class="memory-value">
        <input
          :id="id"
          type="number"
          :value="modelValue"
          :aria-label="label"
          min="512"
          max="262144"
          step="1"
          inputmode="numeric"
          :disabled="disabled"
          @input="type"
        ><span>MiB</span>
      </div>
    </div>
  </div>
</template>
<style scoped>
.memory-field{min-width:0}.memory-control{display:grid;grid-template-columns:minmax(100px,1fr) 132px;gap:18px;align-items:center}.memory-slider-area{min-width:0}.memory-field :deep(.memory-slider){position:relative;display:flex;align-items:center;width:100%;height:24px;touch-action:none;user-select:none}.memory-field :deep(.memory-slider-track){position:relative;flex:1;height:5px;border-radius:999px;background:var(--line)}.memory-field :deep(.memory-slider-range){position:absolute;height:100%;border-radius:inherit;background:var(--accent)}.memory-field :deep(.memory-slider-thumb){display:block;width:17px;height:17px;border:2px solid var(--accent);border-radius:50%;background:var(--panel);box-shadow:0 1px 4px #0002;outline:none;transition:box-shadow .12s ease}.memory-field :deep(.memory-slider-thumb:focus-visible){box-shadow:0 0 0 4px var(--tint);outline:2px solid var(--accent);outline-offset:3px}.memory-field :deep(.memory-slider[data-disabled]){opacity:.45}.memory-limits{display:flex;justify-content:space-between;gap:8px;color:var(--muted);font-size:10px;line-height:1.3}.memory-value{display:flex;align-items:center;gap:7px;min-width:0}.memory-field .memory-value input{width:100%;min-width:0;min-height:40px;padding:9px 10px;border:1px solid var(--line);border-radius:8px;color:var(--ink);background:var(--panel);font:inherit;font-size:13px;box-sizing:border-box}.memory-value>span{font-size:12px;color:var(--muted)}@media(max-width:420px){.memory-control{gap:12px;grid-template-columns:minmax(80px,1fr) 120px}}@media(prefers-reduced-motion:reduce){.memory-field :deep(.memory-slider-thumb){transition:none}}@media(forced-colors:active){.memory-field :deep(.memory-slider-track){background:GrayText}.memory-field :deep(.memory-slider-range){background:Highlight}.memory-field :deep(.memory-slider-thumb){background:Canvas;border-color:Highlight}}
</style>
