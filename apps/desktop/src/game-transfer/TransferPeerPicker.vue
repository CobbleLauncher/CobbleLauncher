<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { Circle, CircleCheck, Monitor } from '@lucide/vue';
import MeshIdentityAvatar from '../mesh-ui/MeshIdentityAvatar.vue';
import { transferText as text } from './labels';
import type { TransferPeer } from './types';
defineProps<{ peers:TransferPeer[];modelValue:string;disabled?:boolean }>();
defineEmits<{'update:modelValue':[value:string]}>();
</script>
<template>
  <fieldset
    class="transfer-peer-picker"
    :disabled="disabled"
  >
    <legend class="sr-only">
      {{ text('chooseTarget') }}
    </legend>
    <label
      v-for="peer in peers"
      :key="peer.peerId"
      :class="{selected:peer.peerId===modelValue}"
    >
      <input
        type="radio"
        name="game-transfer-peer"
        :value="peer.peerId"
        :checked="peer.peerId===modelValue"
        @change="$emit('update:modelValue',peer.peerId)"
      >
      <MeshIdentityAvatar :peer-id="peer.peerId" /><span class="transfer-peer-name"><strong>{{ peer.displayName }}</strong><span><Monitor
        :size="13"
        aria-hidden="true"
      />{{ text(peer.connected?'connected':'notConnected') }}</span></span>
      <CircleCheck
        v-if="peer.peerId===modelValue"
        :size="21"
        aria-hidden="true"
      /><Circle
        v-else
        :size="21"
        aria-hidden="true"
      />
    </label>
  </fieldset>
</template>
<style scoped>
.transfer-peer-picker{margin:0;padding:0;border:0;display:grid;gap:var(--ui-space-2)}.transfer-peer-picker label{position:relative;display:flex;align-items:center;gap:var(--ui-space-4);padding:var(--ui-space-4);border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);cursor:pointer}.transfer-peer-picker label:hover{background:var(--ui-surface-2)}.transfer-peer-picker label.selected{border-color:var(--ui-focus);background:var(--tint)}.transfer-peer-picker label:focus-within{outline:2px solid var(--ui-accent);outline-offset:3px}.transfer-peer-picker input{position:absolute;opacity:0;width:1px;height:1px}.transfer-peer-picker .mesh-identity-avatar{width:46px;height:46px}.transfer-peer-name{display:grid;gap:var(--ui-space-1);flex:1;min-width:0}.transfer-peer-name strong{font-size:var(--ui-text-label);overflow-wrap:anywhere}.transfer-peer-name>span{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-caption);color:var(--ui-text-muted)}.transfer-peer-picker label>svg:last-child{flex:none;color:var(--ui-focus)}
</style>
