<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import AppIcon from '../ui/AppIcon.vue';
import MeshIdentityAvatar from '../mesh-ui/MeshIdentityAvatar.vue';
import { groupText as text } from './labels';
import { groupFreeSlots, type MeshGroupSnapshot } from './types';
const props = defineProps<{ group: MeshGroupSnapshot; names: Record<string, string>; selfPeerId: string; disabled: boolean }>();
defineEmits<{ kick: [peerId: string] }>();
const freeSlots = computed(() => groupFreeSlots(props.group));
const name = (id: string) => props.names[id] || text('peer', { id: id.slice(0, 10) });
</script>
<template>
  <section class="group-members">
    <header>
      <h3>{{ text(group.membership_complete ? 'members' : 'connections') }}</h3>
      <span v-if="freeSlots !== null">{{ text(freeSlots === 1 ? 'freeSlot' : freeSlots ? 'freeSlots' : 'full', { count: freeSlots }) }}</span>
    </header>
    <ul>
      <li
        v-for="member in group.members"
        :key="member.peer_id"
      >
        <MeshIdentityAvatar :peer-id="member.peer_id" />
        <div
          class="group-member-name"
          :title="member.peer_id"
        >
          <strong>{{ name(member.peer_id) }}</strong>
          <span v-if="member.is_host || member.peer_id === selfPeerId">{{ [member.is_host ? text('hostBadge') : '', member.peer_id === selfPeerId ? text('you') : ''].filter(Boolean).join(' · ') }}</span>
        </div>
        <DropdownMenuRoot v-if="group.role === 'host' && !member.is_host && member.peer_id !== selfPeerId">
          <DropdownMenuTrigger
            class="group-member-more"
            :disabled="disabled"
            :aria-label="text('memberActions', { name: name(member.peer_id) })"
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
                @select="$emit('kick', member.peer_id)"
              >
                <AppIcon name="x" />{{ text('remove') }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
      </li>
    </ul>
  </section>
</template>
<style scoped>
.group-members{margin-top:24px}.group-members header{display:flex;align-items:center;justify-content:space-between;gap:12px}.group-members h3{font-size:14px;font-weight:600;margin:0}.group-members header>span{font-size:12px;color:var(--ui-text-muted)}.group-members ul{list-style:none;margin:12px 0 0;padding:0}.group-members li{display:flex;align-items:center;gap:12px;padding:14px 0;border-top:1px solid var(--ui-border)}.group-members :deep(.mesh-identity-avatar){width:38px;height:38px}.group-member-name{display:grid;gap:4px;flex:1;min-width:0}.group-member-name strong{font-size:14px;font-weight:500;overflow-wrap:anywhere}.group-member-name span{font-size:12px;color:var(--ui-text-muted)}.group-member-more{display:grid;place-items:center;width:34px;height:34px;padding:0;border:0;border-radius:8px;background:transparent;color:var(--ui-text-muted)}.group-member-more:hover{background:var(--ui-surface-2)}@media(max-width:480px){.group-members header{align-items:start;flex-direction:column;gap:6px}}
</style>
<style scoped>
.group-members header>span,.group-member-name span{font-size:12px}.group-member-name strong{font-size:14px}
</style>
