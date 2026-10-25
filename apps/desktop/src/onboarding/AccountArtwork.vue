<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
defineProps<{ kind: 'microsoft' | 'offline' | 'complete' }>();
</script>
<template>
  <svg
    class="account-art"
    viewBox="0 0 240 180"
    fill="none"
    aria-hidden="true"
  >
    <ellipse
      cx="120"
      cy="158"
      rx="80"
      ry="10"
      fill="currentColor"
      opacity=".08"
    />
    <template v-if="kind === 'microsoft'">
      <rect
        x="29"
        y="28"
        width="182"
        height="120"
        rx="14"
        fill="var(--panel)"
        stroke="currentColor"
        stroke-width="2"
      />
      <path
        d="M30 54h180"
        stroke="currentColor"
        opacity=".2"
      />
      <circle
        v-for="x in [44, 54, 64]"
        :key="x"
        :cx="x"
        cy="41"
        r="2"
        fill="currentColor"
        opacity=".4"
      />
      <rect
        x="58"
        y="75"
        width="42"
        height="42"
        rx="12"
        fill="currentColor"
        opacity=".12"
      />
      <circle
        cx="79"
        cy="89"
        r="7"
        stroke="currentColor"
        stroke-width="2"
      />
      <path
        d="M66 109c0-13 26-13 26 0"
        stroke="currentColor"
        stroke-width="2"
      />
      <path
        d="M115 85h61m-61 13h44"
        stroke="currentColor"
        stroke-width="4"
        stroke-linecap="round"
        opacity=".25"
      />
      <rect
        x="115"
        y="116"
        width="61"
        height="12"
        rx="6"
        fill="currentColor"
        opacity=".2"
      />
      <circle
        cx="197"
        cy="139"
        r="23"
        fill="var(--accent)"
        stroke="var(--panel)"
        stroke-width="5"
      />
      <path
        d="m187 139 7 7 13-15"
        stroke="var(--accent-ink)"
        stroke-width="3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else>
      <rect
        x="57"
        y="18"
        width="126"
        height="138"
        rx="20"
        fill="var(--panel)"
        stroke="currentColor"
        stroke-width="2"
      />
      <path
        d="M101 18v-6h38v6"
        stroke="currentColor"
        stroke-width="3"
        stroke-linecap="round"
      />
      <rect
        x="82"
        y="42"
        width="76"
        height="67"
        rx="14"
        fill="currentColor"
        opacity=".12"
      />
      <path
        d="M104 58h32v26h-32z"
        fill="currentColor"
        opacity=".65"
      />
      <path
        d="M94 103v-12h52v12"
        fill="currentColor"
        opacity=".4"
      />
      <path
        d="M95 126h50m-38 12h26"
        stroke="currentColor"
        stroke-width="4"
        stroke-linecap="round"
        opacity=".25"
      />
      <circle
        v-if="kind === 'complete'"
        cx="178"
        cy="132"
        r="25"
        fill="var(--accent)"
        stroke="var(--panel)"
        stroke-width="5"
      />
      <path
        v-if="kind === 'complete'"
        d="m167 132 8 8 14-16"
        stroke="var(--accent-ink)"
        stroke-width="3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        v-else
        d="m41 75-9 9 9 9m158-18 9 9-9 9"
        stroke="currentColor"
        stroke-width="2"
        opacity=".4"
      />
    </template>
  </svg>
</template>
<style scoped>
.account-art{width:100%;height:100%;color:var(--ui-focus);stroke-width:1.5;overflow:visible}
</style>
