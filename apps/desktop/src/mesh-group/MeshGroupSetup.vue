<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { MeshHostOptions } from '../mesh';
import AppIcon from '../ui/AppIcon.vue';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import { validGroupPort } from './types';
import { groupText as text } from './labels';
const props = defineProps<{ running: boolean; active: boolean; disabled: boolean; hostOptions: MeshHostOptions | null; initialInvitation?:string }>();
const emit = defineEmits<{ create: [port?: number]; join: [invitation: string]; prepare: [invitation: string]; refresh: []; launch: [] }>();
const mode = ref<'host' | 'join'>('host'), invitation = ref('');
const manualOpen = ref(false), manualPort = ref(''), portEdited = ref(false);
watch(() => props.initialInvitation, value => { if (value?.trim()) { invitation.value = value; mode.value = 'join'; } }, { immediate: true });
const selectedPort = computed(() => manualOpen.value ? Number(manualPort.value) : props.hostOptions?.lan.state === 'detected' ? props.hostOptions.lan.port : undefined);
const canCreate = computed(() => validGroupPort(selectedPort.value));
watch(() => props.hostOptions?.lan.port, port => { if (!portEdited.value) manualPort.value = port ? String(port) : ''; }, { immediate: true });
function join() { const value = invitation.value.trim(); invitation.value = ''; if (value) emit('join', value); }
function changeTab(event: KeyboardEvent) { if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return; event.preventDefault(); mode.value = event.key === 'Home' ? 'host' : event.key === 'End' ? 'join' : mode.value === 'host' ? 'join' : 'host'; (event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role=tab]')[mode.value === 'host' ? 0 : 1]?.focus(); }
watch(() => props.active, active => { if (!active) invitation.value = ''; });
</script>
<template>
  <section class="group-setup">
    <template v-if="!running">
      <p>{{ text('startFirst') }}</p><button
        class="primary-button"
        :disabled="disabled"
        @click="$emit('launch')"
      >
        <AppIcon name="play" />{{ text('startGame') }}
      </button>
    </template>
    <template v-else>
      <div
        class="group-mode"
        role="tablist"
        :aria-label="text('title')"
        @keydown="changeTab"
      >
        <button
          id="mesh-host-tab"
          role="tab"
          aria-controls="mesh-host-panel"
          :aria-selected="mode === 'host'"
          :tabindex="mode === 'host' ? 0 : -1"
          @click="mode = 'host'"
        >
          <AppIcon name="network" />{{ text('host') }}
        </button><button
          id="mesh-join-tab"
          role="tab"
          aria-controls="mesh-join-panel"
          :aria-selected="mode === 'join'"
          :tabindex="mode === 'join' ? 0 : -1"
          @click="mode = 'join'"
        >
          <AppIcon name="plus" />{{ text('join') }}
        </button>
      </div>
      <div
        v-if="mode === 'host'"
        id="mesh-host-panel"
        class="group-host-setup"
        role="tabpanel"
        aria-labelledby="mesh-host-tab"
      >
        <p v-if="hostOptions?.lan.state !== 'detected'">
          {{ text('lanGuide') }}
        </p><div class="group-lan-state">
          <AppIcon :name="hostOptions?.lan.state === 'detected' ? 'check' : 'info'" /><span>{{ hostOptions?.lan.state === 'detected' && hostOptions.lan.port ? text('lanDetected', { port: hostOptions.lan.port }) : text('waitingLan') }}</span><button
            class="text-button"
            :disabled="disabled"
            @click="$emit('refresh')"
          >
            <AppIcon name="refresh-cw" />{{ text('refresh') }}
          </button>
        </div>
        <SettingsDisclosure
          class="group-manual-port"
          :title="text('enterPort')"
          icon="keyboard"
          :value="selectedPort ? String(selectedPort) : undefined"
          @update:open="manualOpen = $event"
        >
          <input
            id="mesh-group-port"
            v-model="manualPort"
            :aria-label="text('port')"
            type="number"
            min="1024"
            max="65535"
            step="1"
            inputmode="numeric"
            :disabled="disabled"
            :aria-invalid="manualOpen && manualPort !== '' && !canCreate"
            :aria-describedby="manualOpen && manualPort !== '' && !canCreate ? 'mesh-group-port-error' : undefined"
            @input="portEdited = true"
          >
          <p
            v-if="manualOpen && manualPort !== '' && !canCreate"
            id="mesh-group-port-error"
            class="group-port-error"
          >
            {{ text('invalidPort') }}
          </p>
        </SettingsDisclosure>
        <button
          class="primary-button"
          :disabled="disabled || !canCreate"
          @click="$emit('create', manualOpen ? selectedPort ?? undefined : undefined)"
        >
          <AppIcon name="plus" />{{ text('create') }}
        </button>
      </div>
      <form
        v-else
        id="mesh-join-panel"
        class="group-join-setup"
        role="tabpanel"
        aria-labelledby="mesh-join-tab"
        @submit.prevent="join"
      >
        <label for="mesh-group-invitation">{{ text('invitation') }}</label><textarea
          id="mesh-group-invitation"
          v-model="invitation"
          :placeholder="text('pasteInvitation')"
          :disabled="disabled"
          maxlength="8192"
          rows="4"
          spellcheck="false"
          autocomplete="off"
        /><button
          class="secondary-button"
          type="button"
          :disabled="disabled || !invitation.trim()"
          @click="$emit('prepare', invitation.trim())"
        >
          <AppIcon name="file-search" />{{ text('prepareEnvironment') }}
        </button><button
          class="primary-button"
          :disabled="disabled || !invitation.trim()"
          type="submit"
        >
          <AppIcon name="network" />{{ text('joinAction') }}
        </button>
      </form>
    </template>
  </section>
</template>
<style scoped>
.group-setup{border:1px solid var(--ui-border);border-radius:14px;background:var(--ui-surface-1);padding:24px}.group-setup>p{font-size:14px;color:var(--ui-text-muted);margin-bottom:20px}.group-mode{display:flex;gap:8px;border-bottom:1px solid var(--ui-border);margin-bottom:24px}.group-mode button{border:0;border-bottom:3px solid transparent;border-radius:0;padding:10px 14px;background:transparent;color:var(--ui-text-muted);font-size:13px}.group-mode [aria-selected=true]{border-bottom-color:var(--ui-accent);color:var(--ui-text);font-weight:600}.group-host-setup>p{max-width:540px;font-size:14px;line-height:1.8;margin:14px 0 20px}.group-lan-icon{color:var(--ui-text-muted)}.group-lan-state{display:flex;align-items:center;gap:8px;font-size:12px;color:var(--ui-text-muted);flex-wrap:wrap;margin-bottom:24px}.group-lan-state .text-button{margin-left:auto}.group-join-setup{display:grid;gap:12px}.group-join-setup label{font-size:13px}.group-join-setup textarea{box-sizing:border-box;width:100%;min-width:0;border:1px solid var(--ui-border);border-radius:9px;padding:12px;background:var(--ui-surface-2);color:var(--ui-text);font:12px/1.6 var(--font-mono,monospace);resize:vertical}.group-setup .primary-button{width:fit-content;display:inline-flex;align-items:center;gap:8px}.group-setup textarea:focus-visible{outline:2px solid var(--ui-focus);outline-offset:2px}@media(max-width:500px){.group-setup{padding:18px}.group-mode button{padding:10px 12px}.group-lan-state .text-button{margin-left:0}}
</style>
<style scoped>
.group-mode button,.group-join-setup>button,.group-lan-state button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.group-mode button>svg{width:16px;height:16px}.group-lan-state{font-size:var(--ui-text-caption)}.group-join-setup>button{justify-self:start}
</style>
<style scoped>
.group-mode button,.group-join-setup label,.group-setup button{font-size:var(--ui-text-label)}.group-lan-state,.group-join-setup textarea{font-size:var(--ui-text-caption)}.group-join-setup textarea{font-family:var(--ui-font-mono)}
.group-manual-port{margin:-12px 0 20px;max-width:320px}.group-manual-port label{display:block;font-size:14px;margin:8px 0}.group-manual-port input{box-sizing:border-box;width:100%;min-width:0;min-height:40px;padding:8px 12px;border:1px solid var(--ui-border);border-radius:8px;background:var(--ui-surface-2);color:var(--ui-text);font:inherit;font-size:14px}.group-manual-port input:focus-visible{outline:2px solid var(--ui-focus);outline-offset:2px}.group-port-error{color:var(--ui-danger);font-size:14px;margin:8px 0}
</style>
