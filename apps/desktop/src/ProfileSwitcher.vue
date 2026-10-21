<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';
import type { ProfileLibrary } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; ready: boolean }>();
defineEmits<{ select: [id: string]; add: []; manage: [] }>();
const active = computed(() => props.library.profiles.find((profile) => profile.id === props.library.active_id));
</script>
<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger
      class="account-button"
      :disabled="busy || !ready"
      aria-label="切换玩家档案"
    >
      <span
        class="profile-avatar small"
        aria-hidden="true"
      >{{ active?.name.slice(0, 1).toUpperCase() || '?' }}</span>
      <span>{{ active?.name || '选择玩家档案' }}</span><span
        v-if="active"
        class="offline-badge"
      >离线</span><span
        class="profile-chevron"
        aria-hidden="true"
      >⌄</span>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        class="profile-menu"
        :side-offset="8"
        align="end"
        :collision-padding="12"
      >
        <p class="profile-menu-label">
          玩家档案
        </p>
        <div class="profile-menu-list">
          <DropdownMenuItem
            v-for="profile in library.profiles"
            :key="profile.id"
            class="profile-menu-item"
            :disabled="busy"
            @select="$emit('select', profile.id)"
          >
            <span
              class="profile-avatar small"
              aria-hidden="true"
            >{{ profile.name.slice(0, 1).toUpperCase() }}</span><span class="profile-menu-name">{{ profile.name }}<small v-if="profile.note">{{ profile.note }}</small></span><span
              v-if="profile.id === library.active_id"
              class="profile-check"
              aria-label="当前使用"
            >✓</span>
          </DropdownMenuItem>
        </div>
        <p
          v-if="!library.profiles.length"
          class="profile-menu-empty"
        >
          还没有玩家档案
        </p>
        <DropdownMenuSeparator class="profile-menu-separator" />
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('add')"
        >
          <span aria-hidden="true">＋</span>添加档案
        </DropdownMenuItem>
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('manage')"
        >
          管理档案<span aria-hidden="true">↗</span>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
