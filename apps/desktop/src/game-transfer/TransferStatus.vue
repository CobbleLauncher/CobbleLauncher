<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { Check, CircleAlert, LoaderCircle } from '@lucide/vue';
import TransferWorldScene from '../ui/TransferWorldScene.vue';
import { transferText as text, formatTransferBytes } from './labels';
import type { TransferRecord } from './types';
const props=defineProps<{record:TransferRecord;sourceLabel:string;targetLabel:string}>();
const active=computed(()=>['queued','waiting','running','cancelling'].includes(props.record.state));
const known=computed(()=>props.record.state==='running'&&typeof props.record.bytesTotal==='number'&&props.record.bytesTotal>0);
const ratio=computed(()=>known.value?Math.max(0,Math.min(1,props.record.bytesDone/props.record.bytesTotal!)):null);
const title=computed(()=>text(props.record.state==='succeeded'?(props.record.direction==='send'?'sent':'received'):props.record.state==='cancelled'?'cancelled':props.record.state==='cancelling'?'cancelling':props.record.state==='interrupted'?'interrupted':props.record.state==='failed'?'failed':props.record.state==='running'?(props.record.phase==='verifying'?'verifying':props.record.phase==='preparing'?'runtimePreparing':props.record.direction==='send'?'sending':'receiving'):'waiting'));
const sceneState=computed(()=>props.record.state==='succeeded'?'complete':props.record.state==='cancelled'?'cancelled':['failed','interrupted'].includes(props.record.state)?'failed':props.record.state==='running'&&!['verifying','preparing'].includes(props.record.phase??'')?'working':'waiting');
</script>
<template>
  <div class="transfer-status">
    <div class="transfer-status-title">
      <Check
        v-if="record.state==='succeeded'"
        :size="25"
        aria-hidden="true"
      /><LoaderCircle
        v-else-if="active"
        class="transfer-status-spin"
        :size="25"
        aria-hidden="true"
      /><CircleAlert
        v-else
        :size="25"
        aria-hidden="true"
      /><h2>{{ title }}</h2>
    </div>
    <TransferWorldScene
      :progress="ratio"
      :state="sceneState"
      :source-label="sourceLabel"
      :target-label="targetLabel"
    />
    <div
      v-if="active"
      class="transfer-meter"
      role="status"
      aria-live="polite"
    >
      <progress
        v-if="known"
        :value="ratio??0"
        max="1"
        :aria-label="title"
      /><progress
        v-else
        :aria-label="title"
      />
      <span v-if="known">{{ formatTransferBytes(record.bytesDone) }} / {{ formatTransferBytes(record.bytesTotal!) }}<strong>{{ Math.floor((ratio??0)*100) }}%</strong></span><span v-else>{{ title }}</span>
    </div>
    <p
      v-if="!active"
      class="transfer-source-kept"
    >
      {{ text('sourceKept') }}
    </p>
  </div>
</template>
<style scoped>
.transfer-status-title{display:flex;justify-content:center;align-items:center;gap:var(--ui-space-3);color:var(--ui-focus)}.transfer-status-title h2{color:var(--ui-text);font-size:var(--ui-text-page);margin:0;font-weight:650}.transfer-status .transfer-world-scene{max-width:850px;margin:var(--ui-space-5) auto}.transfer-meter{max-width:560px;margin:auto;display:grid;gap:var(--ui-space-3)}.transfer-meter progress{width:100%;height:8px;accent-color:var(--ui-focus)}.transfer-meter span{display:flex;gap:var(--ui-space-4);justify-content:space-between;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}.transfer-meter strong{color:var(--ui-text);font-variant-numeric:tabular-nums}.transfer-source-kept{text-align:center;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}.transfer-status-spin{animation:transfer-spin 1.2s linear infinite}@keyframes transfer-spin{to{transform:rotate(360deg)}}@media(prefers-reduced-motion:reduce){.transfer-status-spin{animation:none}}[data-motion='reduced'] .transfer-status-spin{animation:none}
</style>
