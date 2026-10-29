<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import AppIcon from '../ui/AppIcon.vue';
import { t } from '../i18n';
defineEmits<{create:[]; importPack:[]; importLauncher:[]; receiveGame:[]}>();
</script>
<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger class="secondary-button home-add">
      <AppIcon name="plus" />{{ t('home.add') }}<AppIcon
        name="chevron-down"
        :size="14"
      />
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        class="profile-menu home-add-menu"
        align="end"
        :side-offset="7"
        :collision-padding="12"
      >
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('create')"
        >
          <AppIcon name="plus" />{{ t('home.create') }}
        </DropdownMenuItem>
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('importPack')"
        >
          <AppIcon name="puzzle" />{{ t('home.importPack') }}
        </DropdownMenuItem>
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('importLauncher')"
        >
          <AppIcon name="folder" />{{ t('home.importLauncher') }}
        </DropdownMenuItem>
        <DropdownMenuItem
          class="profile-menu-item"
          @select="$emit('receiveGame')"
        >
          <AppIcon name="download" />{{ t('home.receiveGame') }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
<style scoped>.home-add{display:flex;align-items:center;gap:6px;min-height:var(--ui-control-height);font-size:var(--ui-text-caption);padding:4px 8px;border:0;background:transparent;color:var(--ui-text-muted)}</style>
