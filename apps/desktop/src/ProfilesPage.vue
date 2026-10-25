<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref, watch } from 'vue';
import { t, type TranslationKey } from './i18n';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { avatars } from './appearance';
import AppIcon from './ui/AppIcon.vue';
import ProfileAvatar from './ProfileAvatar.vue';
import ProfileSkinPanel from './ProfileSkinPanel.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import { type ProfileLibrary, type PlayerProfile } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; error: string; ready: boolean; choosing: boolean }>();
defineEmits<{ add: []; select: [id: string]; remove: [id: string]; note: [id: string, note: string | null]; retry: [] }>();
const editing = ref<string | null>(null);
const note = ref('');
const removing = ref<string | null>(null);
const viewingSkin = ref<string | null>(null);
const refreshing = ref<string | null>(null);
const feedback = ref<{ id: string; key: TranslationKey; failed?: boolean } | null>(null);
watch(() => props.busy, (busy, previous) => { if (previous && !busy && !props.error) editing.value = null; });
function edit(profile: PlayerProfile) { editing.value = profile.id; note.value = profile.note || ''; removing.value = null; }
async function refreshAppearance(profile: PlayerProfile) {
  refreshing.value = profile.id;
  feedback.value = { id: profile.id, key: 'profiles.refreshingAppearance' };
  try {
    const appearance = await invoke<{ skin: { avatar: string } | null }>('get_profile_appearance', { id: profile.id, refresh: true });
    if (appearance?.skin) avatars.set(profile.id, appearance.skin.avatar);
    else avatars.delete(profile.id);
    feedback.value = { id: profile.id, key: 'profiles.appearanceRefreshed' };
  } catch {
    feedback.value = { id: profile.id, key: 'profiles.errors.appearanceRefresh', failed: true };
  } finally {
    refreshing.value = null;
  }
}
async function copyUuid(profile: PlayerProfile) {
  feedback.value = null;
  try {
    await writeText(profile.uuid);
    feedback.value = { id: profile.id, key: 'profiles.uuidCopied' };
  } catch {
    feedback.value = { id: profile.id, key: 'profiles.errors.uuidCopy', failed: true };
  }
}
function profileError(error: string) {
  const known: Record<string, string> = {
    '无法读取玩家档案，请重试': t('profiles.errors.read'),
    '档案已保存，请重新选择游戏使用的档案。': t('profiles.errors.saved'),
    '请在桌面应用中保存玩家档案': t('profiles.errors.desktopSave'),
    '未能保存档案，请重试': t('profiles.errors.save'),
  };
  return known[error] ?? error;
}
</script>
<template>
  <div class="page-content profiles-page">
    <div
      v-if="library.profiles.length || choosing"
      class="profile-page-tools"
    >
      <p
        v-if="choosing"
        class="create-description"
      >
        {{ t('profiles.chooseForGame') }}
      </p><button
        class="secondary-button"
        :disabled="busy || !ready"
        @click="$emit('add')"
      >
        <AppIcon name="plus" /> {{ t('profiles.addProfile') }}
      </button>
    </div>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ profileError(error) }}
    </p><button
      v-if="!ready && error"
      class="text-button"
      :disabled="busy"
      @click="$emit('retry')"
    >
      {{ t('profiles.reload') }}
    </button>
    <LoadingIndicator
      v-if="!ready && !error"
      :label="t('profiles.loading')"
      skeleton
    />
    <ul
      v-if="ready && library.profiles.length"
      class="profile-list"
      :aria-label="t('profiles.listLabel')"
    >
      <li
        v-for="profile in library.profiles"
        :key="profile.id"
        class="profile-row"
      >
        <div class="profile-row-main">
          <ProfileAvatar
            :id="profile.id"
            :name="profile.name"
            class="profile-avatar"
          /><div class="profile-row-info">
            <h2>{{ profile.name }} <span class="offline-badge">{{ t(profile.kind === 'microsoft' ? 'profiles.kindMicrosoft' : 'profiles.kindOffline') }}</span></h2><p v-if="profile.note">
              {{ profile.note }}
            </p><span
              v-if="profile.id === library.active_id"
              class="profile-current"
            ><AppIcon name="check" /> {{ t('profiles.current') }}</span>
          </div><button
            v-if="profile.id !== library.active_id || choosing"
            class="secondary-button"
            :disabled="busy"
            :aria-label="t('profiles.useNamed', { name: profile.name })"
            @click="$emit('select', profile.id)"
          >
            {{ t('profiles.use') }}
          </button><DropdownMenuRoot>
            <DropdownMenuTrigger
              class="profile-more"
              :disabled="busy"
              :aria-label="t('profiles.manageNamed', { name: profile.name })"
            >
              <AppIcon name="more-horizontal" />
            </DropdownMenuTrigger><DropdownMenuPortal>
              <DropdownMenuContent
                class="profile-menu"
                align="end"
                :side-offset="4"
              >
                <DropdownMenuItem
                  class="profile-menu-item"
                  @select="viewingSkin = viewingSkin === profile.id ? null : profile.id"
                >
                  {{ t('profiles.skin') }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  v-if="profile.kind === 'microsoft'"
                  class="profile-menu-item"
                  :disabled="refreshing === profile.id"
                  @select="refreshAppearance(profile)"
                >
                  <AppIcon name="refresh-cw" /> {{ refreshing === profile.id ? t('profiles.refreshingAppearance') : t('profiles.refreshAppearance') }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  class="profile-menu-item"
                  @select="copyUuid(profile)"
                >
                  <AppIcon name="copy" /> {{ t('profiles.copyUuid') }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  class="profile-menu-item"
                  @select="edit(profile)"
                >
                  {{ t('profiles.editNote') }}
                </DropdownMenuItem><DropdownMenuItem
                  class="profile-menu-item danger-text"
                  @select="removing = profile.id; editing = null"
                >
                  {{ t('profiles.remove') }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
        </div>
        <ProfileSkinPanel
          v-if="viewingSkin === profile.id"
          :profile="profile"
        />
        <p
          v-if="feedback?.id === profile.id"
          class="profile-action-feedback"
          :class="{ 'danger-text': feedback.failed }"
          :role="feedback.failed ? 'alert' : 'status'"
        >
          {{ t(feedback.key) }}
        </p>
        <form
          v-if="editing === profile.id"
          class="profile-inline-form"
          @submit.prevent="$emit('note', profile.id, note.trim() || null)"
        >
          <label :for="`note-${profile.id}`">{{ t('profiles.note') }}</label><input
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
              {{ t('profiles.saveNote') }}
            </button><button
              class="text-button"
              type="button"
              :disabled="busy"
              @click="editing = null"
            >
              {{ t('profiles.cancel') }}
            </button>
          </div>
        </form>
        <div
          v-if="removing === profile.id"
          class="profile-remove-confirm"
        >
          <p>{{ t('profiles.confirmRemove', { name: profile.name }) }}</p><div class="profile-form-actions">
            <button
              class="secondary-button danger-text"
              :disabled="busy"
              @click="$emit('remove', profile.id)"
            >
              {{ t('profiles.confirm') }}
            </button><button
              class="text-button"
              :disabled="busy"
              @click="removing = null"
            >
              {{ t('profiles.cancel') }}
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
      ><AppIcon
        name="plus"
        :size="24"
      /></span><h2>{{ t('profiles.emptyTitle') }}</h2><p>{{ t('profiles.emptyHint') }}</p><button
        class="primary-button"
        :disabled="busy"
        @click="$emit('add')"
      >
        {{ t('profiles.addProfile') }}
      </button>
    </div>
  </div>
</template>
<style scoped>
.profiles-page{font-size:var(--ui-text-body,14px);line-height:calc(var(--font-size-base,14px)*22/14)}.profile-page-tools{justify-content:flex-end}.profile-page-tools .create-description{margin-right:auto}.profiles-page :deep(button),.profiles-page .profile-row-info h2,.profiles-page .profiles-empty h2{font-size:var(--ui-text-body,14px);line-height:calc(var(--font-size-base,14px)*22/14)}.profile-row-info h2{display:flex;flex-wrap:wrap;align-items:center;gap:6px}.profile-row-info h2 .offline-badge{display:inline-block;margin-left:0;font-size:var(--ui-text-caption,12px);line-height:calc(var(--font-size-base,14px)*18/14)}.profiles-page .profile-row-info p,.profiles-page .profile-current,.profiles-page .profiles-empty p{font-size:var(--ui-text-caption,12px);line-height:calc(var(--font-size-base,14px)*18/14)}.profile-action-feedback{margin:8px 0 0 58px;font-size:var(--ui-text-caption,12px);line-height:calc(var(--font-size-base,14px)*18/14)}
@media(max-width:560px){.profile-action-feedback{margin-left:0}}
</style>
