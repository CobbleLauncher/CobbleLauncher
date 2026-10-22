<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref, watch } from 'vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import { profileKindLabel, type ProfileLibrary, type PlayerProfile } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; error: string; ready: boolean; choosing: boolean }>();
defineEmits<{ add: []; select: [id: string]; remove: [id: string]; note: [id: string, note: string | null]; retry: [] }>();
const editing = ref<string | null>(null);
const note = ref('');
const removing = ref<string | null>(null);
watch(() => props.busy, (busy, previous) => { if (previous && !busy && !props.error) editing.value = null; });
function edit(profile: PlayerProfile) { editing.value = profile.id; note.value = profile.note || ''; removing.value = null; }
</script>
<template>
  <div class="page-content profiles-page">
    <div class="profile-page-tools">
      <p class="create-description">
        {{ choosing ? '选择本次游戏要使用的身份，之后返回游戏。' : '' }}
      </p><button
        class="secondary-button"
        :disabled="busy || !ready"
        @click="$emit('add')"
      >
        ＋ 添加档案
      </button>
    </div>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p><button
      v-if="!ready"
      class="text-button"
      @click="$emit('retry')"
    >
      重新读取档案
    </button>
    <ul
      v-if="library.profiles.length"
      class="profile-list"
      aria-label="玩家档案"
    >
      <li
        v-for="profile in library.profiles"
        :key="profile.id"
        class="profile-row"
      >
        <div class="profile-row-main">
          <span
            class="profile-avatar"
            aria-hidden="true"
          >{{ profile.name.slice(0, 1).toUpperCase() }}</span><div class="profile-row-info">
            <h2>{{ profile.name }} <span class="offline-badge">{{ profileKindLabel(profile) }}</span></h2><p v-if="profile.note">
              {{ profile.note }}
            </p><span
              v-if="profile.id === library.active_id"
              class="profile-current"
            >✓ 当前使用</span>
          </div><button
            v-if="profile.id !== library.active_id || choosing"
            class="secondary-button"
            :disabled="busy"
            :aria-label="`使用 ${profile.name}`"
            @click="$emit('select', profile.id)"
          >
            使用
          </button><DropdownMenuRoot>
            <DropdownMenuTrigger
              class="profile-more"
              :disabled="busy"
              :aria-label="`管理 ${profile.name}`"
            >
              •••
            </DropdownMenuTrigger><DropdownMenuPortal>
              <DropdownMenuContent
                class="profile-menu"
                align="end"
                :side-offset="4"
              >
                <DropdownMenuItem
                  class="profile-menu-item"
                  @select="edit(profile)"
                >
                  修改备注
                </DropdownMenuItem><DropdownMenuItem
                  class="profile-menu-item danger-text"
                  @select="removing = profile.id; editing = null"
                >
                  移除档案
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
        </div>
        <form
          v-if="editing === profile.id"
          class="profile-inline-form"
          @submit.prevent="$emit('note', profile.id, note.trim() || null)"
        >
          <label :for="`note-${profile.id}`">备注</label><input
            :id="`note-${profile.id}`"
            v-model="note"
            maxlength="160"
            :disabled="busy"
          ><div class="profile-form-actions">
            <button
              class="secondary-button"
              type="submit"
              :disabled="busy"
            >
              保存备注
            </button><button
              class="text-button"
              type="button"
              :disabled="busy"
              @click="editing = null"
            >
              取消
            </button>
          </div>
        </form>
        <div
          v-if="removing === profile.id"
          class="profile-remove-confirm"
        >
          <p>移除档案 {{ profile.name }}？游戏和存档会保留。</p><div class="profile-form-actions">
            <button
              class="secondary-button danger-text"
              :disabled="busy"
              @click="$emit('remove', profile.id)"
            >
              确认移除
            </button><button
              class="text-button"
              :disabled="busy"
              @click="removing = null"
            >
              取消
            </button>
          </div>
        </div>
      </li>
    </ul>
    <div
      v-else-if="ready"
      class="profiles-empty"
    >
      <span
        class="profile-avatar large"
        aria-hidden="true"
      >＋</span><h2>还没有玩家档案</h2><p>安装和管理游戏无需身份，开始游戏前再添加。</p><button
        class="primary-button"
        :disabled="busy"
        @click="$emit('add')"
      >
        添加档案
      </button>
    </div>
  </div>
</template>
