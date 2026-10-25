<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue';
import { SliderRoot, SliderTrack, SliderRange, SliderThumb } from 'reka-ui';

const props = withDefaults(defineProps<{
  id?: string;
  label: string;
  modelValue: number;
  min: number;
  max: number;
  step?: number;
  defaultValue?: number;
  disabled?: boolean;
  valueLabel?: string;
  valueText?: string;
  unit?: string;
  stops?: readonly number[];
}>(), { id: undefined, step: 1, defaultValue: undefined, disabled: false, valueLabel: undefined, valueText: undefined, unit: undefined, stops: undefined });
const emit = defineEmits<{ 'update:modelValue': [value: number] }>();
const generatedId = useId();
const inputId = computed(() => props.id ?? `${generatedId}-number`);
const sliderStep = computed(() => props.step > 0 && Number.isFinite(props.step) ? props.step : 1);
function normalize(value: number): number {
  const bounded = Math.max(props.min, Math.min(props.max, value));
  const snapped = props.min + Math.round((bounded - props.min) / sliderStep.value) * sliderStep.value;
  return Math.max(props.min, Math.min(props.max, Number(snapped.toFixed(8))));
}
const value = computed(() => normalize(Number.isFinite(props.modelValue) ? props.modelValue : props.min));
const stops = computed(() => props.stops && props.stops.length > 1 ? props.stops : undefined);
function positionFor(value: number): number {
  return stops.value ? stops.value.reduce((closest, amount, index) => Math.abs(amount - value) < Math.abs(stops.value![closest]! - value) ? index : closest, 0) : value;
}
const sliderValue = computed(() => positionFor(value.value));
const sliderMin = computed(() => stops.value ? 0 : props.min);
const sliderMax = computed(() => stops.value ? stops.value.length - 1 : props.max);
const draft = ref(String(value.value));
let pendingValue: number | undefined;
watch(value, next => { draft.value = String(next); pendingValue = undefined; });
const markerPosition = computed(() => {
  if (props.defaultValue === undefined || props.defaultValue < props.min || props.defaultValue > props.max || props.max <= props.min) return undefined;
  return `${100 * (positionFor(props.defaultValue) - sliderMin.value) / (sliderMax.value - sliderMin.value)}%`;
});
function update(next: number) {
  if (props.disabled) return;
  const bounded = normalize(next);
  draft.value = String(bounded);
  if (bounded !== props.modelValue && bounded !== pendingValue) {
    pendingValue = bounded;
    emit('update:modelValue', bounded);
  }
}
function typeInput(event: Event) {
  if (props.disabled) return;
  draft.value = (event.target as HTMLInputElement).value;
  const next = Number(draft.value), position = (next - props.min) / sliderStep.value;
  if (draft.value.trim() !== '' && Number.isFinite(next) && next >= props.min && next <= props.max && Math.abs(position - Math.round(position)) < 1e-8) update(next);
}
function updateSlider(next: number[] | undefined) {
  if (next?.[0] !== undefined) {
    const mapped = stops.value ? stops.value[Math.round(next[0])] : next[0];
    if (mapped !== undefined) update(mapped);
  }
}
function commitInput() {
  const next = Number(draft.value);
  if (draft.value.trim() === '' || !Number.isFinite(next)) draft.value = String(value.value);
  else update(next);
}
</script>

<template>
  <div class="numeric-slider-field">
    <div class="numeric-slider-controls">
      <div class="numeric-slider-track-area">
        <SliderRoot
          class="numeric-slider"
          :model-value="[sliderValue]"
          :min="sliderMin"
          :max="sliderMax"
          :step="stops ? 1 : sliderStep"
          :disabled="disabled"
          @update:model-value="updateSlider"
        >
          <SliderTrack class="numeric-slider-track">
            <SliderRange class="numeric-slider-range" />
            <span
              v-if="markerPosition"
              class="numeric-slider-default"
              :style="{ left: markerPosition }"
              :data-default-value="defaultValue"
              aria-hidden="true"
            />
          </SliderTrack>
          <SliderThumb
            class="numeric-slider-thumb"
            :aria-label="label"
            :aria-valuetext="valueText ?? (unit ? `${value}${unit === '%' ? '' : ' '}${unit}` : undefined)"
          />
        </SliderRoot>
        <div
          v-if="$slots.markers"
          class="numeric-slider-markers"
        >
          <slot
            name="markers"
            :default-value="defaultValue"
            :position="markerPosition"
          />
        </div>
      </div>
      <span
        v-if="valueText !== undefined"
        class="numeric-slider-value-text"
      >{{ valueText }}</span>
      <div
        v-else
        class="numeric-slider-value"
      >
        <input
          :id="inputId"
          class="numeric-slider-input"
          :value="draft"
          type="number"
          inputmode="numeric"
          :aria-label="valueLabel ?? label"
          :min="min"
          :max="max"
          :step="sliderStep"
          :disabled="disabled"
          @input="typeInput"
          @change="commitInput"
          @blur="commitInput"
          @keydown.enter.prevent="commitInput"
          @keydown.esc.prevent="draft = String(value)"
        >
        <span
          v-if="unit"
          class="numeric-slider-unit"
          aria-hidden="true"
        >{{ unit }}</span>
      </div>
    </div>
    <slot />
  </div>
</template>

<style scoped>
.numeric-slider-field{min-width:0;width:100%}
.numeric-slider-controls{display:grid;grid-template-columns:minmax(0,1fr) max-content;align-items:center;gap:var(--ui-space-4)}
.numeric-slider-track-area{min-width:0}
.numeric-slider{position:relative;display:flex;align-items:center;width:100%;height:var(--ui-control-height);touch-action:none;user-select:none}
.numeric-slider-track{position:relative;flex:1;height:4px;border-radius:var(--ui-radius-round);background:var(--ui-border-strong)}
.numeric-slider-range{position:absolute;height:100%;border-radius:inherit;background:var(--ui-focus)}
.numeric-slider-default{position:absolute;top:-4px;width:2px;height:12px;margin-left:-1px;background:var(--ui-text-muted);pointer-events:none}
.numeric-slider-thumb{position:relative;display:block;width:20px;height:20px;border:2px solid var(--ui-focus);border-radius:var(--ui-radius-round);background:var(--ui-surface-1);cursor:grab;transition:background-color var(--ui-duration-fast)}
.numeric-slider-thumb:active{cursor:grabbing;background:var(--tint)}
.numeric-slider-thumb:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}
.numeric-slider[data-disabled]{opacity:.5;cursor:not-allowed}
.numeric-slider[data-disabled] .numeric-slider-thumb{cursor:not-allowed}
.numeric-slider-value{position:relative;display:flex;align-items:center;min-width:0;width:var(--numeric-slider-value-width,88px)}
.numeric-slider-value-text{font-size:var(--ui-text-label);line-height:var(--ui-leading-control);color:var(--ui-text);text-align:right}
.numeric-slider-value input{box-sizing:border-box;width:100%;min-width:0;min-height:var(--ui-control-height);padding:var(--ui-space-1) var(--ui-space-2);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:var(--ui-text-label);line-height:var(--ui-leading-control);font-variant-numeric:tabular-nums}
.numeric-slider-value:has(.numeric-slider-unit) input{padding-right:28px;appearance:textfield}
.numeric-slider-value:has(.numeric-slider-unit) input::-webkit-inner-spin-button{appearance:none}
.numeric-slider-unit{position:absolute;right:var(--ui-space-2);font-size:var(--ui-text-caption);color:var(--ui-text-muted);pointer-events:none}
.numeric-slider-markers{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}
</style>
