<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import AppIcon from '../ui/AppIcon.vue';
const props=defineProps<{value?:string;label:string}>();
const tone=computed(()=>['verified','authenticated','online','running','available'].includes(props.value??'')?'good':['failed','recovery_failed'].includes(props.value??'')?'bad':'neutral');
</script>
<template>
  <span
    class="mesh-evidence-badge"
    :data-tone="tone"
  ><AppIcon :name="tone==='good'?'check':tone==='bad'?'alert-circle':value==='testing'?'loader-circle':'info'" />{{ label }}</span>
</template>
<style scoped>
.mesh-evidence-badge{display:inline-flex;align-items:center;gap:var(--ui-space-1);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted);white-space:nowrap}.mesh-evidence-badge svg{width:14px;height:14px;flex:none}.mesh-evidence-badge[data-tone=good]{color:var(--ui-success)}.mesh-evidence-badge[data-tone=bad]{color:var(--ui-danger)}
</style>
