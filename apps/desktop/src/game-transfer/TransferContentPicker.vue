<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { Archive, Blocks, Brush, FileCog, Image, SlidersHorizontal } from '@lucide/vue';
import { transferText as text } from './labels';
import { transferScopes, type TransferScope, type TransferSelection } from './types';
const props=defineProps<{available:TransferScope[];modelValue:TransferSelection;disabled?:boolean}>();
const emit=defineEmits<{'update:modelValue':[value:TransferSelection]}>();
const icons={saves:Archive,mods:Blocks,resourcepacks:Image,shaderpacks:Brush,config:FileCog,options:SlidersHorizontal};
function update(scope:TransferScope,event:Event){emit('update:modelValue',{...props.modelValue,[scope]:(event.target as HTMLInputElement).checked});}
</script>
<template>
  <fieldset
    class="transfer-content-picker"
    :disabled="disabled"
  >
    <legend class="sr-only">
      {{ text('chooseContent') }}
    </legend>
    <label
      v-for="scope in transferScopes.filter(item=>available.includes(item))"
      :key="scope"
      :class="{selected:modelValue[scope]}"
    >
      <component
        :is="icons[scope]"
        :size="21"
        :stroke-width="1.6"
        aria-hidden="true"
      />
      <span>{{ text(scope) }}</span><input
        type="checkbox"
        :checked="modelValue[scope]"
        @change="update(scope,$event)"
      >
    </label>
  </fieldset>
</template>
<style scoped>
.transfer-content-picker{margin:0;padding:0;border:0;display:grid;grid-template-columns:1fr 1fr;gap:0 var(--ui-space-5)}.transfer-content-picker label{display:flex;align-items:center;gap:var(--ui-space-3);min-height:64px;border-bottom:1px solid var(--ui-border);cursor:pointer;color:var(--ui-text-muted);padding:var(--ui-space-3) 0}.transfer-content-picker label.selected{color:var(--ui-text)}.transfer-content-picker label>span{flex:1;font-size:var(--ui-text-body)}.transfer-content-picker input{accent-color:var(--ui-focus);width:18px;height:18px;flex:none}.transfer-content-picker:disabled label{cursor:default;opacity:.6}@media(max-width:520px){.transfer-content-picker{grid-template-columns:1fr}}
</style>
