<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ArrowLeft, ArrowRight, Check, FolderInput, FolderOpen, HardDrive } from '@lucide/vue';
import TransferWorldScene from '../ui/TransferWorldScene.vue';
import ConfirmationDialog from '../ui/ConfirmationDialog.vue';
import { locale } from '../i18n';
import { isActiveTask, taskFailureMessage, taskStateLabel, type TaskSnapshot } from '../tasks';
const props = defineProps<{ initialTaskId?: string }>();
const emit = defineEmits<{ completed: []; back: [] }>();
type Preview = { planId: string; source: string; target: string; gameCount: number; requiredBytes: number; availableBytes: number | null; enoughSpace: boolean | null };
type Retained = { source: string; target: string; sourceRetained: boolean; canRecycle: boolean };
const preview = ref<Preview | null>(null), task = ref<TaskSnapshot | null>(null), retained = ref<Retained | null>(null);
const locations = ref<{ source: string; target: string } | null>(null);
const busy = ref(false), error = ref(''), confirm = ref(false), mounted = ref(true);
type Stage = 'choose' | 'review' | 'transfer' | 'complete';
const stage = ref<Stage>('choose');
const selectedId = ref(props.initialTaskId);
const stageHeading = ref<HTMLElement | null>(null);
const stages = computed(() => zh.value ? ['选择位置', '确认', '搬迁', '完成'] : ['Location', 'Review', 'Move', 'Done']);
const stageIndex = computed(() => ['choose', 'review', 'transfer', 'complete'].indexOf(stage.value));
const heading = computed(() => zh.value ? { choose: '选择新位置', review: '确认搬迁', transfer: task.value ? taskStateLabel(task.value) : '准备搬迁', complete: '搬迁完成' }[stage.value] : { choose: 'Choose a new location', review: 'Review the move', transfer: task.value ? taskStateLabel(task.value) : 'Preparing the move', complete: 'Move complete' }[stage.value]);
const zh = computed(() => locale.value === 'zh-CN');
const active = computed(() => !!task.value && isActiveTask(task.value));
const blocked = computed(() => busy.value || active.value || !isTauri());
const percent = computed(() => task.value?.progress?.total ? Math.min(100, Math.floor(task.value.progress.completed / task.value.progress.total * 100)) : null);
const waitingLabel = computed(() => task.value?.state === 'queued' ? (zh.value ? '等待其他任务完成' : 'Waiting for other tasks') : task.value?.state === 'cancelling' ? (zh.value ? '正在取消…' : 'Cancelling…') : (zh.value ? '正在检测文件…' : 'Checking files…'));
const sceneState = computed(() => {
  const state = task.value?.state;
  if (state === 'succeeded') return 'complete';
  if (state === 'failed' || state === 'interrupted') return 'failed';
  if (state === 'cancelled') return 'cancelled';
  return state === 'running' ? 'working' : 'waiting';
});
function bytes(value: number) { return new Intl.NumberFormat(locale.value, { style: 'unit', unit: value >= 1073741824 ? 'gigabyte' : 'megabyte', maximumFractionDigits: 1 }).format(value / (value >= 1073741824 ? 1073741824 : 1048576)); }
function message(cause: unknown) {
  const code = String(cause);
  if (/NO_SPACE/u.test(code)) return zh.value ? '目标空间不足，请更换位置。' : 'Not enough space. Choose another location.';
  if (/GAME_BUSY|MIGRATION_BUSY/u.test(code)) return zh.value ? '请先关闭游戏并等待当前操作完成。' : 'Close the games and wait for current operations to finish.';
  if (/CHANGED|SOURCE_IN_USE/u.test(code)) return zh.value ? '文件或游戏位置已变化，原文件保留。请检查两处目录。' : 'Files or game locations changed. The originals are kept. Check both folders.';
  if (/EXISTS|TARGET/u.test(code)) return zh.value ? '目标已存在或与原位置重叠，请更换位置。' : 'The destination exists or overlaps the original. Choose another location.';
  if (/LINK/u.test(code)) return zh.value ? '目录包含链接，无法迁移。' : 'The folder contains links and cannot be moved.';
  if (/LIMIT/u.test(code)) return zh.value ? '目录文件过多，无法迁移。原文件保留。' : 'The folder has too many files to move. The originals are kept.';
  if (/EXPIRED/u.test(code)) return zh.value ? '请重新选择位置。' : 'Choose the location again.';
  if (/RECYCLE/u.test(code)) return zh.value ? '无法移入回收站，原文件保留。' : 'Could not move the originals to the Recycle Bin. They are kept.';
  return zh.value ? '操作未完成，请重试。原文件保留。' : 'The operation did not finish. Retry. The originals are kept.';
}
async function inspect(id: string) {
  try { const value = await invoke<Retained>('inspect_game_directory_migration', { taskId: id }); if (mounted.value && task.value?.id === id) { retained.value = value; locations.value = value; } }
  catch (cause) { if (mounted.value) error.value = message(cause); }
}
function update(value: TaskSnapshot, announce = true) {
  if (!selectedId.value && !task.value && announce) return;
  if (!mounted.value || value.kind !== 'directory_migration' || selectedId.value && value.id !== selectedId.value || task.value && value.id !== task.value.id) return;
  if (task.value?.id === value.id && value.updated_at < task.value.updated_at) return;
  const previous = task.value?.state;
  task.value = value;
  stage.value = value.state === 'succeeded' ? 'complete' : 'transfer';
  if (previous === undefined && value.state !== 'succeeded' || value.state === 'failed' || value.state === 'interrupted') void inspect(value.id);
  if (value.state === 'succeeded' && previous !== 'succeeded') { void inspect(value.id); if (announce) emit('completed'); }
}
async function pick() {
  if (blocked.value) return;
  busy.value = true; error.value = '';
  try { const value = await invoke<Preview | null>('preview_game_directory_migration'); if (value && mounted.value) { preview.value = value; locations.value = value; task.value = null; retained.value = null; selectedId.value = undefined; stage.value = 'review'; } }
  catch (cause) { error.value = message(cause); }
  finally { busy.value = false; }
}
async function start() {
  if (!preview.value || blocked.value || preview.value.enoughSpace === false) return;
  busy.value = true; error.value = '';
  try {
    const id = await invoke<string>('start_game_directory_migration', { planId: preview.value.planId });
    selectedId.value = id;
    const records = await invoke<TaskSnapshot[]>('list_tasks');
    const value = records.find(item => item.id === id); if (value) update(value);
    preview.value = null;
  } catch (cause) { error.value = message(cause); }
  finally { busy.value = false; }
}
async function control(command: 'cancel_task' | 'retry_task') {
  if (!task.value || busy.value) return;
  busy.value = true; error.value = '';
  try { update(await invoke<TaskSnapshot>(command, { taskId: task.value.id })); }
  catch (cause) { error.value = message(cause); }
  finally { busy.value = false; }
}
async function original(cleanup = false) {
  if (!task.value || busy.value) return;
  busy.value = true; error.value = '';
  try { await invoke(cleanup ? 'cleanup_game_directory_migration_source' : 'open_game_directory_migration_source', { taskId: task.value.id }); if (cleanup) await inspect(task.value.id); }
  catch (cause) { error.value = message(cause); }
  finally { busy.value = false; }
}
let unlisten: (() => void) | undefined;
async function restore(id?: string) {
  try {
    const records = await invoke<TaskSnapshot[]>('list_tasks');
    if (!mounted.value || !Array.isArray(records) || id !== selectedId.value) return;
    const value = [...records].sort((left, right) => right.updated_at - left.updated_at || right.created_at - left.created_at).find(item => item.kind === 'directory_migration' && (id ? item.id === id : isActiveTask(item)));
    if (value) update(value, false);
    else if (id) error.value = zh.value ? '找不到这次搬迁。' : 'This move is no longer available.';
  } catch (cause) { if (mounted.value) error.value = message(cause); }
}
watch(() => props.initialTaskId, id => { selectedId.value = id; task.value = null; retained.value = null; preview.value = null; locations.value = null; stage.value = 'choose'; void restore(id); });
onMounted(async () => {
  if (!isTauri()) return;
  try {
    const stop = await listen<TaskSnapshot>('task-updated', event => update(event.payload));
    if (!mounted.value) { stop(); return; } unlisten = stop;
    await restore(props.initialTaskId);
  } catch { /* Existing settings remain usable when task history is unavailable. */ }
});
onUnmounted(() => { mounted.value = false; unlisten?.(); });
</script>
<template>
  <section class="migration-page page-content">
    <ol
      class="migration-steps"
      :aria-label="zh ? '搬迁步骤' : 'Move steps'"
    >
      <li
        v-for="(label, index) in stages"
        :key="label"
        :class="{ current: index === stageIndex, finished: index < stageIndex }"
        :aria-current="index === stageIndex ? 'step' : undefined"
      >
        <span class="migration-step-number"><Check
          v-if="index < stageIndex"
          :size="14"
          aria-hidden="true"
        /><template v-else>{{ index + 1 }}</template></span><span>{{ label }}</span>
      </li>
    </ol>
    <section
      class="migration-workspace"
      :aria-busy="busy"
    >
      <Transition
        name="migration-stage"
        mode="out-in"
        @after-enter="stageHeading?.focus()"
      >
        <div
          :key="stage"
          class="migration-stage-content"
        >
          <h2
            ref="stageHeading"
            tabindex="-1"
          >
            {{ heading }}
          </h2>
          <template v-if="stage === 'choose'">
            <div class="migration-destination">
              <HardDrive
                :size="36"
                :stroke-width="1.5"
                aria-hidden="true"
              />
              <p
                v-if="preview"
                class="migration-folder"
              >
                {{ preview.target }}
              </p>
              <button
                type="button"
                :class="preview ? 'secondary-button' : 'primary-button'"
                :disabled="blocked"
                @click="pick"
              >
                <FolderInput
                  :size="17"
                  aria-hidden="true"
                />{{ busy ? (zh ? '正在检测…' : 'Checking…') : preview ? (zh ? '更换文件夹' : 'Change folder') : (zh ? '选择文件夹' : 'Choose folder') }}
              </button>
            </div>
            <p class="migration-note">
              {{ zh ? '存档和配置一起搬迁。' : 'Worlds and settings move with your games.' }}
            </p>
          </template>
          <template v-else-if="stage === 'review' && preview">
            <dl class="migration-paths">
              <div><dt>{{ zh ? '原位置' : 'From' }}</dt><dd>{{ preview.source }}</dd></div>
              <ArrowRight
                class="migration-path-arrow"
                :size="18"
                aria-hidden="true"
              />
              <div><dt>{{ zh ? '新位置' : 'To' }}</dt><dd>{{ preview.target }}</dd></div>
            </dl>
            <dl class="migration-capacity">
              <div><dt>{{ zh ? '游戏' : 'Games' }}</dt><dd>{{ preview.gameCount }}</dd></div>
              <div><dt>{{ zh ? '需要空间' : 'Required' }}</dt><dd>{{ bytes(preview.requiredBytes) }}</dd></div>
              <div><dt>{{ zh ? '目标可用' : 'Available' }}</dt><dd>{{ preview.availableBytes === null ? (zh ? '未知' : 'Unknown') : bytes(preview.availableBytes) }}</dd></div>
            </dl>
            <p
              v-if="preview.enoughSpace === false"
              role="alert"
              class="setting-error"
            >
              {{ zh ? '目标空间不足，请更换位置。' : 'Not enough space. Choose another location.' }}
            </p>
            <p class="migration-note">
              {{ zh ? '验证后切换游戏位置。原文件保留，完成后可清理。' : 'Switch locations after verification. Originals are kept until you clean them up.' }}
            </p>
          </template>
          <template v-else-if="stage === 'transfer'">
            <TransferWorldScene
              :progress="percent === null ? null : percent / 100"
              :state="sceneState"
              :source-label="zh ? '原位置' : 'Original'"
              :target-label="zh ? '新位置' : 'New location'"
            />
            <div
              class="migration-progress"
              role="status"
            >
              <div
                v-if="active"
                class="migration-progress-caption"
              >
                <span>{{ percent === null ? waitingLabel : `${bytes(task?.progress?.completed ?? 0)} / ${bytes(task?.progress?.total ?? 0)}` }}</span><strong v-if="percent !== null">{{ percent }}%</strong>
              </div>
              <progress
                v-if="active"
                :value="percent ?? undefined"
                max="100"
                :aria-label="zh ? '搬迁进度' : 'Move progress'"
              />
              <p
                v-if="task?.error && task.state !== 'cancelled'"
                class="setting-error"
              >
                {{ task.error.includes('DIRECTORY_MIGRATION_') ? message(task.error) : taskFailureMessage(task) }}
              </p>
              <p
                v-if="task?.state === 'cancelled'"
                class="migration-note"
              >
                {{ zh ? '原文件保留。' : 'Originals are kept.' }}
              </p>
            </div>
          </template>
          <template v-else-if="stage === 'complete'">
            <TransferWorldScene
              :progress="1"
              state="complete"
              :source-label="zh ? '原位置' : 'Original'"
              :target-label="zh ? '新位置' : 'New location'"
            />
            <dl
              v-if="locations"
              class="migration-paths migration-result-paths"
            >
              <div><dt>{{ zh ? '游戏位置' : 'Game location' }}</dt><dd>{{ locations.target }}</dd></div>
            </dl>
            <p class="migration-note">
              {{ retained?.sourceRetained === false ? (zh ? '原目录已移入回收站。' : 'The original folder is in the Recycle Bin.') : (zh ? '原文件保留。检查新位置后，可清理原目录。' : 'Originals are kept. Check the new location before cleaning them up.') }}
            </p>
          </template>
          <p
            v-if="error"
            role="alert"
            class="setting-error migration-error"
          >
            {{ error }}
          </p>
        </div>
      </Transition>
      <footer
        v-if="stage !== 'choose' || preview"
        class="migration-footer"
        :class="{ 'migration-footer-forward': stage === 'choose' }"
      >
        <template v-if="stage === 'choose'">
          <button
            v-if="preview"
            type="button"
            class="primary-button"
            :disabled="busy"
            @click="stage = 'review'"
          >
            {{ zh ? '继续' : 'Continue' }}<ArrowRight
              :size="16"
              aria-hidden="true"
            />
          </button>
        </template>
        <template v-else-if="stage === 'review'">
          <button
            type="button"
            class="text-button"
            :disabled="busy"
            @click="stage = 'choose'"
          >
            <ArrowLeft
              :size="16"
              aria-hidden="true"
            />{{ zh ? '上一步' : 'Back' }}
          </button>
          <button
            v-if="preview?.enoughSpace === false"
            type="button"
            class="secondary-button"
            :disabled="busy"
            @click="pick"
          >
            {{ zh ? '更换位置 / 重新检测' : 'Change location / recheck' }}
          </button>
          <button
            v-else
            type="button"
            class="primary-button"
            :disabled="blocked"
            @click="start"
          >
            {{ busy ? (zh ? '准备中…' : 'Preparing…') : (zh ? '开始搬迁' : 'Start move') }}<ArrowRight
              :size="16"
              aria-hidden="true"
            />
          </button>
        </template>
        <template v-else-if="stage === 'transfer'">
          <button
            v-if="active"
            type="button"
            class="text-button"
            :disabled="busy || task?.state === 'cancelling'"
            @click="control('cancel_task')"
          >
            {{ zh ? '取消搬迁' : 'Cancel move' }}
          </button>
          <button
            v-else
            type="button"
            class="text-button"
            :disabled="busy"
            @click="stage = 'choose'"
          >
            <ArrowLeft
              :size="16"
              aria-hidden="true"
            />{{ zh ? '更换位置' : 'Choose another location' }}
          </button>
          <button
            v-if="active"
            type="button"
            class="secondary-button"
            @click="emit('back')"
          >
            {{ zh ? '后台继续' : 'Continue in background' }}
          </button>
          <button
            v-else-if="task?.can_retry"
            type="button"
            class="primary-button"
            :disabled="busy"
            @click="control('retry_task')"
          >
            {{ zh ? '重试' : 'Retry' }}
          </button>
          <button
            v-else
            type="button"
            class="secondary-button"
            @click="emit('back')"
          >
            {{ zh ? '返回' : 'Back' }}
          </button>
        </template>
        <template v-else>
          <div class="migration-original-actions">
            <button
              v-if="retained?.sourceRetained"
              type="button"
              class="text-button"
              :disabled="busy"
              @click="original()"
            >
              <FolderOpen
                :size="16"
                aria-hidden="true"
              />{{ zh ? '检查原位置' : 'Check original location' }}
            </button>
            <button
              v-if="retained?.sourceRetained && retained.canRecycle"
              type="button"
              class="secondary-button"
              :disabled="busy"
              @click="confirm = true"
            >
              {{ zh ? '清理原位置' : 'Clean up original location' }}
            </button>
          </div>
          <button
            type="button"
            class="primary-button"
            @click="emit('back')"
          >
            {{ zh ? '完成' : 'Done' }}<Check
              :size="16"
              aria-hidden="true"
            />
          </button>
        </template>
      </footer>
    </section>
    <ConfirmationDialog
      v-model:open="confirm"
      :title="zh ? '清理原位置？' : 'Clean up the original location?'"
      :description="`${retained?.source ?? ''}\n${zh ? '核验两处文件未变后，将原目录移入回收站。清空回收站后才会释放空间。' : 'After checking both copies are unchanged, move the original folder to the Recycle Bin. Empty the Recycle Bin to free disk space.'}`"
      :confirm-label="zh ? '移入回收站' : 'Move to Recycle Bin'"
      danger
      require-delay
      :busy="busy"
      @confirm="original(true)"
    />
  </section>
</template>
<style scoped>
.migration-page{width:100%;max-width:960px;margin-inline:auto;display:flex;flex-direction:column;gap:var(--ui-space-6);min-height:min(600px,calc(100dvh - 230px))}.migration-steps{list-style:none;display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:var(--ui-space-3);margin:0;padding:0}.migration-steps li{position:relative;display:flex;align-items:center;gap:var(--ui-space-2);color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.migration-steps li:not(:last-child)::after{content:'';height:1px;flex:1;background:var(--ui-border);margin-inline-start:var(--ui-space-2)}.migration-step-number{width:25px;height:25px;border:1px solid var(--ui-border);border-radius:50%;display:grid;place-items:center;flex:none}.migration-steps .current{color:var(--ui-text);font-weight:var(--ui-weight-semibold)}.current .migration-step-number{background:var(--ui-accent);color:var(--ui-on-accent,#17331b);border-color:transparent}.finished .migration-step-number{color:var(--ui-text);background:var(--ui-surface-2)}.migration-workspace{display:flex;flex:1;flex-direction:column;min-width:0;border-block:1px solid var(--ui-border)}.migration-stage-content{flex:1;min-width:0;padding-block:var(--ui-space-6);display:flex;flex-direction:column;gap:var(--ui-space-6)}.migration-stage-content :deep(.transfer-world-scene){max-width:700px}.migration-stage-content h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:var(--ui-weight-semibold);margin:0}.migration-stage-content p{margin:0}.migration-stage-content h2:focus{outline:none}.migration-footer.migration-footer-forward{justify-content:flex-end}.migration-destination{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:var(--ui-space-5);min-height:270px;color:var(--ui-text-muted)}.migration-destination>svg{padding:18px;box-sizing:content-box;border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-2)}.migration-folder{font-size:var(--ui-text-label);color:var(--ui-text);overflow-wrap:anywhere;max-width:100%;text-align:center}.migration-note,.migration-progress-caption{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.migration-paths{display:grid;grid-template-columns:minmax(0,1fr) 22px minmax(0,1fr);align-items:center;gap:var(--ui-space-5);margin:0;padding:var(--ui-space-5) 0}.migration-paths>div{display:grid;gap:var(--ui-space-2);min-width:0}.migration-paths dt,.migration-capacity dt{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.migration-paths dd{font-size:var(--ui-text-label);line-height:var(--ui-leading-label);margin:0;overflow-wrap:anywhere}.migration-path-arrow{color:var(--ui-text-muted)}.migration-capacity{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));margin:0;border-block:1px solid var(--ui-border);padding-block:var(--ui-space-5);gap:var(--ui-space-5)}.migration-capacity>div{display:grid;gap:var(--ui-space-2)}.migration-capacity dd{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:var(--ui-weight-semibold);margin:0}.migration-progress{display:grid;gap:var(--ui-space-3)}.migration-progress-caption{display:flex;justify-content:space-between;gap:var(--ui-space-3)}.migration-progress-caption strong{color:var(--ui-text);font-weight:var(--ui-weight-semibold)}.migration-progress progress{width:100%;height:6px;accent-color:var(--ui-accent)}.migration-result-paths{grid-template-columns:minmax(0,1fr);padding:0}.migration-footer{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);padding-block:var(--ui-space-5);border-top:1px solid var(--ui-border)}.migration-footer button,.migration-destination button{display:inline-flex;align-items:center;justify-content:center;gap:var(--ui-space-2)}.migration-original-actions{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-3)}.migration-error{font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}.migration-stage-enter-active,.migration-stage-leave-active{transition:opacity var(--ui-duration-fast),transform var(--ui-duration-fast)}.migration-stage-enter-from{opacity:0;transform:translateY(5px)}.migration-stage-leave-to{opacity:0;transform:translateY(-4px)}@media(max-width:600px){.migration-page{gap:var(--ui-space-4);min-height:calc(100dvh - 230px)}.migration-steps{gap:var(--ui-space-2)}.migration-steps li{flex-direction:column;align-items:flex-start;gap:var(--ui-space-2)}.migration-steps li:not(:last-child)::after{position:absolute;top:12px;inset-inline:33px 0;margin:0}.migration-paths{grid-template-columns:minmax(0,1fr);gap:var(--ui-space-4)}.migration-path-arrow{transform:rotate(90deg)}.migration-stage-content{padding-block:var(--ui-space-5);gap:var(--ui-space-5)}.migration-capacity{gap:var(--ui-space-3)}.migration-capacity dd{font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}.migration-footer{flex-wrap:wrap}.migration-original-actions{width:100%;justify-content:space-between;gap:var(--ui-space-2)}.migration-footer>.primary-button:last-child{margin-inline-start:auto}.migration-destination{min-height:230px}}@media(prefers-reduced-motion:reduce){.migration-stage-enter-active,.migration-stage-leave-active{transition:none}}
</style>
