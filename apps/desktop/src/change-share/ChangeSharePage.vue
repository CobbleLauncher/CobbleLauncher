<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Game } from '../games';
import ExportChanges from './ExportChanges.vue';
import ImportChanges from './ImportChanges.vue';
import { label as l } from './labels';
import type { ApplyResult } from './types';
import './change-share.css';

const props = defineProps<{ game: Game; mode: 'export' | 'import'; importedId?: string }>();
const emit = defineEmits<{ back: []; applied: [targetId: string, result: ApplyResult] }>();
const locked = computed(() => props.game.status !== 'installed' || !!props.game.content_installing || props.game.launch_state === 'preparing' || props.game.launch_state === 'running');
const exportPage = ref<InstanceType<typeof ExportChanges> | null>(null);
const importPage = ref<InstanceType<typeof ImportChanges> | null>(null);
function confirmLeave(): Promise<boolean> { return props.mode === 'export' ? exportPage.value?.confirmLeave() ?? Promise.resolve(true) : importPage.value?.confirmLeave() ?? Promise.resolve(true); }
defineExpose({ confirmLeave });
</script>

<template>
  <div class="page-content change-share-page">
    <p class="change-share-game">
      {{ game.name }} <span>{{ game.version }}</span>
    </p>
    <p
      v-if="locked && mode === 'export'"
      class="settings-error"
      role="status"
    >
      {{ l('locked') }}
    </p>
    <ExportChanges
      v-if="mode === 'export'"
      ref="exportPage"
      :game-id="game.id"
      :locked="locked"
    />
    <ImportChanges
      v-else
      ref="importPage"
      :game="game"
      :imported-id="importedId"
      @applied="(targetId, result) => emit('applied', targetId, result)"
    />
  </div>
</template>
