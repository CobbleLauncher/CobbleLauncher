<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ArrowDownToLine, ArrowRight } from '@lucide/vue';
import { formatTransferBytes } from './labels';
import type { TransferOffer,TransferPeer } from './types';
defineProps<{offers:TransferOffer[];peers:TransferPeer[];disabled?:boolean}>();
defineEmits<{select:[offer:TransferOffer]}>();
</script>
<template>
  <div class="transfer-offer-list">
    <button
      v-for="offer in offers"
      :key="`${offer.peerId}:${offer.manifestId}`"
      type="button"
      :disabled="disabled"
      @click="$emit('select',offer)"
    >
      <span class="transfer-offer-icon"><ArrowDownToLine
        :size="24"
        :stroke-width="1.5"
        aria-hidden="true"
      /></span><span class="transfer-offer-title"><strong>{{ offer.gameName||`Minecraft ${offer.version}` }}</strong><span>{{ peers.find(peer=>peer.peerId===offer.peerId)?.displayName||offer.peerId }} · {{ formatTransferBytes(offer.totalBytes) }}</span></span><ArrowRight
        :size="19"
        aria-hidden="true"
      />
    </button>
  </div>
</template>
<style scoped>
.transfer-offer-list{display:grid;gap:var(--ui-space-3)}.transfer-offer-list button{display:flex;align-items:center;gap:var(--ui-space-4);padding:var(--ui-space-4) 0;text-align:start;border:0;border-bottom:1px solid var(--ui-border);background:none;color:var(--ui-text);cursor:pointer;min-width:0}.transfer-offer-list button:hover{color:var(--ui-focus)}.transfer-offer-icon{display:grid;place-items:center;width:52px;height:52px;border-radius:var(--ui-radius-lg);background:var(--tint);color:var(--ui-focus);flex:none}.transfer-offer-title{display:grid;gap:var(--ui-space-1);min-width:0;flex:1}.transfer-offer-title strong{font-size:var(--ui-text-label);overflow-wrap:anywhere}.transfer-offer-title>span{color:var(--ui-text-muted);font-size:var(--ui-text-caption);overflow-wrap:anywhere}.transfer-offer-list button>svg{flex:none}
</style>
