<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import ProfileAvatar from './ProfileAvatar.vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';
import type { ProfileLibrary } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; ready: boolean; beforeOpen?: () => Promise<void> }>();
const emit = defineEmits<{ select: [id: string]; add: []; manage: []; 'open-change': [open: boolean] }>();
const open = ref(false);
let opening = 0;
async function updateOpen(value: boolean) {
  const current = ++opening;
  if (value) {
    try { await props.beforeOpen?.(); } catch { return; }
  }
  if (current !== opening) return;
  open.value = value;
  emit('open-change', value);
}
const active = computed(() => props.library.profiles.find((profile) => profile.id === props.library.active_id));
</script>
<template>
  <DropdownMenuRoot
    :open="open"
    @update:open="updateOpen"
  >
    <DropdownMenuTrigger
      class="account-button"
      :disabled="busy || !ready"
      :aria-label="t('profiles.switch')"
    >
      <ProfileAvatar
        :id="active?.id"
        :name="active?.name"
        class="profile-avatar small"
      />
      <span>{{ active?.name || t('profiles.choose') }}</span><span
        v-if="active"
        class="offline-badge"
      >{{ t(active.kind === 'microsoft' ? 'profiles.kindMicrosoft' : 'profiles.kindOffline') }}</span><AppIcon
        name="chevron-down"
        class="profile-chevron"
      />
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        class="profile-menu"
        :side-offset="4"
        align="end"
        :collision-padding="12"
      >
        <div class="profile-menu-list">
          <DropdownMenuItem
            v-for="profile in library.profiles"
            :key="profile.id"
            class="profile-menu-item"
            :disabled="busy"
            @select="$emit('select', profile.id)"
          >
            <ProfileAvatar
              :id="profile.id"
              :name="profile.name"
              class="profile-avatar small"
            /><span class="profile-menu-name">{{ profile.name }}<small v-if="profile.note">{{ profile.note }}</small></span><span class="offline-badge">{{ t(profile.kind === 'microsoft' ? 'profiles.kindMicrosoft' : 'profiles.kindOffline') }}</span><AppIcon
              v-if="profile.id === library.active_id"
              class="profile-check"
              name="check"
            /><span class="sr-only">{{ t('profiles.current') }}</span>
          </DropdownMenuItem>
        </div>
        <p
          v-if="!library.profiles.length"
          class="profile-menu-empty"
        >
          {{ t('profiles.emptyTitle') }}
        </p>
        <DropdownMenuSeparator class="profile-menu-separator" />
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('add')"
        >
          <AppIcon name="plus" /> {{ t('profiles.addProfile') }}
        </DropdownMenuItem>
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('manage')"
        >
          {{ t('profiles.manage') }}<AppIcon name="external-link" />
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
