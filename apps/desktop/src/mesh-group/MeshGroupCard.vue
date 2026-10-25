<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import { locale } from '../i18n';
import AppIcon from '../ui/AppIcon.vue';
import MeshGroupMembers from './MeshGroupMembers.vue';
import { groupText as text } from './labels';
import { groupActive, type MeshGroupSnapshot } from './types';
const props = defineProps<{ group: MeshGroupSnapshot; names: Record<string, string>; selfPeerId: string; invitation: string; address: string; disabled: boolean; copied: string }>();
defineEmits<{ copy: [kind: 'invitation' | 'address']; invite: []; revoke: []; stop: []; kick: [peerId: string] }>();
const active = computed(() => groupActive(props.group));
const hostName = computed(() => props.names[props.group.host_peer_id] || text('peer', { id: props.group.host_peer_id.slice(0, 10) }));
const expires = computed(() => props.group.invitation_expires_at ? new Date(props.group.invitation_expires_at * 1000).toLocaleTimeString(locale.value, { hour: '2-digit', minute: '2-digit' }) : '');
</script>
<template>
  <section class="mesh-group-card">
    <header class="group-summary">
      <div>
        <h2>{{ text('hostedBy', { name: hostName }) }}</h2><p
          class="group-state"
          :class="{ live: active }"
        >
          <span aria-hidden="true" />{{ text(`state_${group.state}`) }}
        </p>
      </div>
      <DropdownMenuRoot v-if="active && group.role === 'host'">
        <DropdownMenuTrigger
          class="secondary-button group-more"
          :disabled="disabled"
          :aria-label="text('groupActions')"
        >
          <AppIcon name="more-horizontal" />
        </DropdownMenuTrigger>
        <DropdownMenuPortal>
          <DropdownMenuContent
            class="profile-menu"
            align="end"
            :side-offset="6"
            :collision-padding="12"
          >
            <DropdownMenuItem
              class="profile-menu-item"
              :disabled="disabled"
              @select="$emit('invite')"
            >
              <AppIcon name="refresh-cw" />{{ text(group.admission_open ? 'replaceInvite' : 'newInvite') }}
            </DropdownMenuItem>
            <DropdownMenuItem
              class="profile-menu-item"
              :disabled="disabled || !group.admission_open"
              @select="$emit('revoke')"
            >
              <AppIcon name="x" />{{ text('revoke') }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenuPortal>
      </DropdownMenuRoot>
    </header>
    <template v-if="active">
      <MeshGroupMembers
        :group="group"
        :names="names"
        :self-peer-id="selfPeerId"
        :disabled="disabled"
        @kick="$emit('kick', $event)"
      />
      <div class="group-access">
        <div
          v-if="group.role === 'guest' && address"
          class="group-address"
        >
          <label for="group-minecraft-address">{{ text('address') }}</label><div>
            <input
              id="group-minecraft-address"
              :value="address"
              readonly
              spellcheck="false"
            ><button
              class="secondary-button"
              :disabled="disabled"
              @click="$emit('copy', 'address')"
            >
              <AppIcon :name="copied === 'address' ? 'check' : 'copy'" />{{ text(copied === 'address' ? 'copied' : 'copyAddress') }}
            </button>
          </div><p>{{ text('addressGuide') }}</p>
        </div>
        <p
          v-if="group.role === 'host' && !group.admission_open"
          class="group-hint"
        >
          {{ text('inviteClosed') }}
        </p>
        <p
          v-else-if="group.role === 'host' && expires"
          class="group-hint"
        >
          {{ text('inviteExpires', { time: expires }) }}
        </p>
        <div class="group-actions">
          <button
            v-if="group.role === 'host' && invitation && group.admission_open"
            class="primary-button"
            :disabled="disabled"
            @click="$emit('copy', 'invitation')"
          >
            <AppIcon :name="copied === 'invitation' ? 'check' : 'copy'" />{{ text(copied === 'invitation' ? 'copied' : 'copyInvite') }}
          </button>
          <button
            v-else-if="group.role === 'host'"
            class="primary-button"
            :disabled="disabled"
            @click="$emit('invite')"
          >
            <AppIcon name="plus" />{{ text(group.admission_open ? 'replaceInvite' : 'newInvite') }}
          </button>
          <button
            class="text-button group-stop"
            :disabled="disabled"
            @click="$emit('stop')"
          >
            <AppIcon name="x" />{{ text(group.role === 'host' ? 'stop' : 'leave') }}
          </button>
        </div>
      </div>
    </template>
  </section>
</template>
<style scoped>
.mesh-group-card{border:1px solid var(--ui-border);border-radius:14px;background:var(--ui-surface-1);padding:24px}.group-summary{display:flex;align-items:start;justify-content:space-between;gap:16px}.group-summary h2{font-size:14px;letter-spacing:-.2px;line-height:1.4;overflow-wrap:anywhere;margin:0}.group-state{display:flex;align-items:center;gap:7px;color:var(--ui-text-muted);font-size:12px;margin:9px 0 0}.group-state>span{width:6px;height:6px;border-radius:50%;background:var(--ui-text-muted)}.group-state.live>span{background:var(--ui-success)}.group-more{padding:0!important;min-width:34px;min-height:34px!important}.group-hint{font-size:12px;color:var(--ui-text-muted);margin:16px 0 0}.group-actions{display:flex;align-items:center;gap:18px;flex-wrap:wrap;margin-top:22px}.group-stop{margin-left:auto}.group-address{margin-top:22px}.group-address label{display:block;font-size:12px;color:var(--ui-text-muted);margin-bottom:8px}.group-address>div{display:flex;gap:10px}.group-address input{min-width:0;flex:1;padding:10px 12px;border:1px solid var(--ui-border);border-radius:8px;font:13px var(--font-mono,monospace);color:var(--ui-text);background:var(--ui-surface-2)}.group-address p{font-size:12px;line-height:1.6;color:var(--ui-text-muted);margin-top:10px}.group-actions button,.group-address button{display:inline-flex;align-items:center;justify-content:center;gap:8px}@media(max-width:500px){.mesh-group-card{padding:18px}.group-summary h2{font-size:14px}.group-address>div{flex-wrap:wrap}.group-address input{flex-basis:100%}.group-stop{margin-left:0}}
</style>
<style scoped>
.group-summary h2{font-size:var(--ui-text-section);font-weight:var(--ui-weight-semibold)}.group-access{border-inline-start:1px solid var(--ui-border);padding-inline-start:var(--ui-space-5)}.group-address input{font-size:var(--ui-text-section)}.group-stop{color:var(--ui-text-muted)}@media(max-width:760px){.group-access{border-inline-start:0;border-top:1px solid var(--ui-border);padding-inline-start:0;margin-top:var(--ui-space-3)}}
</style>
<style scoped>
.mesh-group-card{display:grid;grid-template-columns:minmax(0,1fr) minmax(250px,.85fr);column-gap:32px}.group-summary{grid-column:1/-1}.group-access{min-width:0;padding-top:24px}.group-address,.group-hint{margin-top:0}.group-stop{margin-left:0}.group-state{font-size:14px}.group-hint,.group-address label,.group-address p{font-size:12px}.group-address input{font-size:var(--ui-text-section);font-family:var(--ui-font-mono)}.group-address>div{flex-wrap:wrap}.group-address input{flex-basis:100%}.group-address button,.group-actions button{font-size:14px}.group-access .group-actions{margin-top:18px}@media(max-width:760px){.mesh-group-card{grid-template-columns:minmax(0,1fr)}.group-access{padding-top:20px}}
</style>
