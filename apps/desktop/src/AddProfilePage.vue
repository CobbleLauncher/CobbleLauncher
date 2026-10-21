<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import type { ProfileLibrary } from './games';
const props = defineProps<{ library: ProfileLibrary; busy: boolean; error: string; ready: boolean }>();
const emit = defineEmits<{ add: [name: string, note: string | null]; select: [id: string]; cancel: [] }>();
const name = ref('');
const note = ref('');
const attempted = ref(false);
const valid = computed(() => /^[A-Za-z0-9_]{3,16}$/.test(name.value));
const duplicate = computed(() => props.library.profiles.find((profile) => profile.name === name.value));
function submit() {
  attempted.value = true;
  if (!valid.value || props.busy || !props.ready) return;
  if (duplicate.value) emit('select', duplicate.value.id);
  else emit('add', name.value, note.value.trim() || null);
}
</script>
<template>
  <div class="page-content profile-add-page">
    <div class="profile-intro">
      <span
        class="profile-avatar large"
        aria-hidden="true"
      >{{ name.slice(0, 1).toUpperCase() || '+' }}</span><div><h2>离线档案</h2></div>
    </div>
    <form
      class="create-form"
      @submit.prevent="submit"
    >
      <div class="form-field">
        <label for="profile-name">游戏昵称</label><input
          id="profile-name"
          v-model="name"
          autocomplete="off"
          spellcheck="false"
          maxlength="16"
          placeholder="例如 Dylan"
          :disabled="busy"
          :aria-invalid="attempted && !valid"
          aria-describedby="profile-name-help"
        ><p
          id="profile-name-help"
          class="setting-hint"
        >
          3–16 位英文字母、数字或下划线，区分大小写。
        </p><p
          v-if="attempted && !valid"
          class="settings-error"
          role="alert"
        >
          昵称须为 3–16 位英文字母、数字或下划线
        </p>
      </div>
      <p
        v-if="duplicate"
        class="profile-notice"
      >
        这个昵称已在档案中。可以直接使用已有身份。
      </p>
      <div
        v-else
        class="form-field"
      >
        <label for="profile-note">备注 <span class="field-note">可选</span></label><input
          id="profile-note"
          v-model="note"
          maxlength="160"
          placeholder="例如 建筑存档使用"
          :disabled="busy"
        >
      </div>
      <details class="profile-explanation">
        <summary>离线身份和存档</summary><p>昵称决定玩家 UUID。不同昵称进入同一个世界时，背包、位置等玩家数据可能不同。昵称创建后不能修改；需要新昵称时，请添加另一个档案。</p><p>档案切换不会复制游戏或隔离存档。</p>
      </details>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <div class="profile-form-actions">
        <button
          class="primary-button"
          type="submit"
          :disabled="busy || !ready"
        >
          {{ busy ? '保存中…' : duplicate ? '使用此档案' : '添加并使用' }}
        </button><button
          class="text-button"
          type="button"
          :disabled="busy"
          @click="$emit('cancel')"
        >
          取消
        </button>
      </div>
    </form>
  </div>
</template>
