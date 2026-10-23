<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { watch } from 'vue';
import { avatars, loadAvatar } from './appearance';
const props = defineProps<{ id?: string; name?: string }>();
watch(() => props.id, id => { if(id)void loadAvatar(id); }, { immediate:true });
</script>
<template>
  <span aria-hidden="true"><img
    v-if="id && avatars.get(id)"
    :src="avatars.get(id)"
    alt=""
  ><template v-else>{{ name?.slice(0,1).toUpperCase() || '?' }}</template></span>
</template>
<style scoped>img{width:100%;height:100%;object-fit:cover;image-rendering:pixelated;border-radius:inherit}</style>
