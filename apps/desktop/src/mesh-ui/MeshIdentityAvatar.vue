<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { meshIdentityDigest, meshIdentityTiles } from '../mesh-social';
const props = defineProps<{ peerId: string }>();
const digest = ref('');
let request = 0;
watch(() => props.peerId, async peer => {
  const current = ++request; digest.value = '';
  try { const value = await meshIdentityDigest(peer); if (request === current) digest.value = value; } catch { /* The full ID remains the identity source. */ }
}, { immediate: true });
const tiles = computed(() => meshIdentityTiles(digest.value));
const hue = computed(() => parseInt(digest.value.slice(0, 4) || '0', 16) % 360);
</script>
<template>
  <svg
    class="mesh-identity-avatar"
    viewBox="-1 -1 8 8"
    aria-hidden="true"
    :style="{ '--identity-hue': String(hue) }"
  >
    <rect
      x="-1"
      y="-1"
      width="8"
      height="8"
      rx="1.6"
      class="avatar-ground"
    />
    <template
      v-for="tile in tiles"
      :key="`${tile.x}:${tile.y}`"
    >
      <path
        v-if="tile.triangle"
        :d="`M${tile.x} ${tile.y}h1v1z`"
      />
      <rect
        v-else
        :x="tile.x"
        :y="tile.y"
        width="1"
        height="1"
        rx=".12"
      />
    </template>
  </svg>
</template>
<style scoped>
.mesh-identity-avatar{width:64px;height:64px;flex:none;stroke:none;fill:hsl(var(--identity-hue) 48% 38%)}.avatar-ground{fill:hsl(var(--identity-hue) 35% 91%)}
</style>
