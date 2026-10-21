<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import type { Game } from './games';
const props = defineProps<{ game: Game; retrying: boolean; hasProfile: boolean; starting: boolean }>();
defineEmits<{ retry: [id: string]; play: [id: string]; manage: [id: string] }>();
const phase = computed(() => ({ metadata: '获取游戏信息', files: '下载游戏文件', assets: '下载游戏资源', fabric: '安装 Fabric', complete: '安装完成' })[props.game.progress?.phase ?? 'metadata']);
const percent = computed(() => {
  const progress = props.game.progress;
  return progress && progress.total > 0 ? Math.min(100, progress.completed / progress.total * 100) : 0;
});
</script>

<template>
  <article
    class="game-card"
    :aria-label="game.name"
  >
    <div
      class="game-cover"
      aria-hidden="true"
    >
      <svg
        viewBox="0 0 80 80"
        stroke="none"
      ><path
        fill="var(--scene-top)"
        d="m12 24 28-16 28 16-28 16z"
      /><path
        fill="var(--scene-left)"
        d="m12 24 28 16v32L12 56z"
      /><path
        fill="var(--scene-right)"
        d="m40 40 28-16v32L40 72z"
      /><path
        fill="var(--stone-left)"
        d="m12 35 28 16v21L12 56z"
      /><path
        fill="var(--stone-right)"
        d="m40 51 28-16v21L40 72z"
      /></svg>
    </div>
    <div class="game-details">
      <div class="game-card-title">
        <h2>{{ game.name }}</h2><span
          v-if="game.status === 'installed'"
          class="installed-label"
        >✓ 已安装</span><span
          v-else-if="game.status === 'failed'"
          class="failed-label"
        >安装未完成</span>
      </div>
      <p class="game-version">
        Minecraft {{ game.version }} <span>· {{ game.loader_version ? `Fabric ${game.loader_version}` : '原版' }}</span>
      </p>
      <template v-if="game.status === 'installing'">
        <div
          class="install-status"
          role="status"
        >
          <span>{{ phase }}</span><span v-if="game.progress && game.progress.total > 0">{{ game.progress.completed }} / {{ game.progress.total }}</span>
        </div>
        <div
          class="install-track"
          role="progressbar"
          :aria-label="phase"
          :aria-valuenow="game.progress?.completed"
          :aria-valuemax="game.progress?.total || undefined"
        >
          <div :style="{ width: `${percent}%` }" />
        </div>
      </template>
      <p
        v-if="game.status === 'failed'"
        class="game-error"
        role="alert"
      >
        {{ game.error || '安装中断，请重试' }}
      </p>
      <p
        v-if="game.status === 'installed'"
        class="game-directory"
        :title="game.directory"
      >
        {{ game.directory }}
      </p>
      <p
        v-if="game.launch_error"
        class="game-error"
        role="alert"
      >
        启动失败：{{ game.launch_error }}
      </p>
      <button
        v-if="game.status === 'installed'"
        class="text-button"
        @click="$emit('manage', game.id)"
      >
        管理内容
      </button>
      <p
        v-if="game.launch_state === 'preparing' || game.launch_state === 'running'"
        class="launch-status"
        role="status"
      >
        {{ game.launch_state === 'running' ? '游戏运行中' : '正在准备启动…' }}
      </p>
    </div>
    <button
      v-if="game.status === 'failed'"
      class="secondary-button retry-button"
      :disabled="retrying"
      @click="$emit('retry', game.id)"
    >
      {{ retrying ? '重试中…' : '重试安装' }}
    </button>
    <button
      v-if="game.status === 'installed'"
      class="primary-button play-button"
      :disabled="starting || game.launch_state === 'preparing' || game.launch_state === 'running'"
      @click="$emit('play', game.id)"
    >
      {{ game.launch_state === 'running' ? '运行中' : game.launch_state === 'preparing' || starting ? '启动中…' : hasProfile ? '开始游戏' : '设置离线档案' }}
    </button>
  </article>
</template>
