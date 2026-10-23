<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import { pluginLocale } from './extensions/contributions';
import ReportResults from './ReportResults.vue';
import InspectorCell from './inspector/InspectorCell.vue';
import InspectorSummary from './inspector/InspectorSummary.vue';
import type { DetailQuery, DetailReply, InspectorField, InspectorPageReply, InspectorQuery, InspectorRow, InspectorSection, InspectorSession } from './inspector/types';
const props = defineProps<{ pluginId: string; instanceId?: string; allowCompare?: boolean; disabled?: boolean }>();
const session = ref<InspectorSession | null>(null);
const response = ref<InspectorPageReply | null>(null);
const opening = ref(false);
const loading = ref(false);
const error = ref('');
const section = ref<InspectorSection>('overview');
const search = ref('');
const offset = ref(0);
const pageSize = ref(50);
const detail = ref<{ row: InspectorRow; field: InspectorField; text: string; total: number; next: number | null } | null>(null);
const detailLoading = ref(false);
const detailError = ref('');
const detailQuery = ref<DetailQuery | null>(null);
let revision = 0;
let disposed = false;
let searchTimer: ReturnType<typeof setTimeout> | undefined;
let pendingOwner: { pluginId: string; instanceId?: string } | null = null;
const detailBytes = computed(() => new TextEncoder().encode(detail.value?.text ?? '').length);
const page = computed(() => response.value?.page);
const findings = computed(() => response.value ? { ...response.value.report, tables: [] } : null);
function closeSession() {
  clearTimeout(searchTimer); ++revision;
  const sessionId = session.value?.sessionId;
  const ownedPending = opening.value;
  const owner = pendingOwner;
  pendingOwner = null;
  session.value = null; response.value = null; detail.value = null; error.value = ''; loading.value = false; opening.value = false;
  if (!sessionId && !ownedPending) return Promise.resolve();
  detailLoading.value = false; detailError.value = '';
  return invoke('close_pack_inspection', sessionId ? { sessionId } : owner ?? { pluginId: props.pluginId, ...(props.instanceId ? { instanceId: props.instanceId } : {}) }).catch(cause => { if (!disposed) error.value = String(cause); });
}
async function begin(compare: boolean) {
  const owner = { pluginId: props.pluginId, ...(props.instanceId ? { instanceId: props.instanceId } : {}) };
  await closeSession();
  if (disposed || owner.pluginId !== props.pluginId || owner.instanceId !== props.instanceId) return;
  pendingOwner = owner;
  opening.value = true; error.value = '';
  const current = ++revision;
  try {
    try {
      const settings = await invoke<{ values: Record<string, unknown> }>('get_extension_settings', { ...owner, locale: pluginLocale() });
      const size = settings?.values?.page_size;
      if ([20, 50, 100].includes(Number(size))) pageSize.value = Number(size);
    } catch { /* Legacy reports have no declared settings. */ }
    if (disposed || current !== revision) return;
    const value = await invoke<InspectorSession | null>('begin_pack_inspection', { ...owner, compare, locale: pluginLocale() });
    if (disposed || current !== revision) {
      if (value) void invoke('close_pack_inspection', { sessionId: value.sessionId }).catch(() => {});
      return;
    }
    if (value) { opening.value = false; session.value = value; section.value = 'overview'; search.value = ''; offset.value = 0; await query(); }
  } catch (cause) { if (!disposed && current === revision) error.value = String(cause); }
  finally { if (!disposed && current === revision) opening.value = false; }
}
async function query() {
  clearTimeout(searchTimer);
  if (!session.value) return;
  const sessionId = session.value.sessionId;
  const current = ++revision;
  const request: InspectorQuery = { section: section.value, search: search.value, offset: offset.value, limit: pageSize.value };
  loading.value = true; error.value = ''; response.value = null; detail.value = null;
  try {
    const value = await invoke<InspectorPageReply | null>('query_pack_inspection', { sessionId, query: request, revision: current, locale: pluginLocale() });
    if (!disposed && current === revision && value?.sessionId === session.value?.sessionId && value.revision === current) response.value = value;
  } catch (cause) { if (!disposed && current === revision) error.value = String(cause); }
  finally { if (!disposed && current === revision) loading.value = false; }
}
function chooseSection(value: InspectorSection) { section.value = value; offset.value = 0; void query(); }
function searchChanged() {
  clearTimeout(searchTimer); ++revision; loading.value = false; response.value = null; detail.value = null; offset.value = 0;
  searchTimer = setTimeout(() => { void query(); }, 250);
}
function paginate(value: number) { offset.value = value; void query(); }
async function readDetail(row: InspectorRow, field: InspectorField, next = 0) {
  if (!session.value) return;
  const sessionId = session.value.sessionId;
  const current = ++revision;
  detailError.value = '';
  const cell = row[field];
  if (!next && cell?.complete) {
    detail.value = {row, field, text: cell.text, total: cell.byte_length, next: null};
    detailLoading.value = false;
    return;
  }
  const request: DetailQuery = { row_id: row.id, field, offset: next, limit: 2048 };
  if (!next) detail.value = { row, field, text: '', total: row[field]?.byte_length ?? 0, next: 0 };
  detailQuery.value = request; detailLoading.value = true; detailError.value = '';
  try {
    const value = await invoke<DetailReply | null>('detail_pack_inspection', { sessionId, query: request, revision: current, locale: pluginLocale() });
    if (!disposed && current === revision && value?.sessionId === session.value?.sessionId && value.revision === current && value.detail.row_id === row.id && value.detail.field === field && value.detail.offset === next) {
      detail.value = { row, field, text: (next ? detail.value?.text ?? '' : '') + value.detail.text, total: value.detail.total_bytes, next: value.detail.next_offset };
    }
  } catch (cause) { if (!disposed && current === revision) detailError.value = String(cause); }
  finally { if (!disposed && current === revision) detailLoading.value = false; }
}
function closeDetail() { ++revision; detail.value = null; detailError.value = ''; detailLoading.value = false; }
watch(() => [props.pluginId, props.instanceId], () => { void closeSession(); });
onUnmounted(() => { disposed = true; void closeSession(); });
</script>
<template>
  <div
    class="inspector-panel"
    :aria-busy="opening || loading"
  >
    <div class="inspector-actions">
      <button
        class="secondary-button"
        :disabled="disabled || opening"
        @click="begin(false)"
      >
        {{ opening ? t('inspector.opening') : t('inspector.open') }}
      </button>
      <button
        v-if="allowCompare"
        class="text-button"
        :disabled="disabled || opening"
        @click="begin(true)"
      >
        {{ t('inspector.compare') }}
      </button>
      <button
        v-if="opening || session"
        class="text-button"
        @click="closeSession"
      >
        {{ opening ? t('inspector.cancel') : t('inspector.close') }}
      </button>
    </div>
    <p
      v-if="!session && !opening && !error"
      class="content-empty"
    >
      {{ t('inspector.unopened') }}
    </p>
    <template v-if="session">
      <InspectorSummary :summary="session.summary" />
      <div
        class="inspector-actions inspector-sections"
        role="group"
        :aria-label="t('inspector.source')"
      >
        <button
          v-for="item in (session.summary.baseline ? ['overview', 'current', 'baseline', 'changes'] : ['overview', 'current']) as InspectorSection[]"
          :key="item"
          class="text-button"
          :aria-pressed="section === item"
          @click="chooseSection(item)"
        >
          {{ t(`inspector.${item}`) }}
        </button>
      </div>
      <form
        class="form-field inspector-search"
        @submit.prevent="query"
      >
        <label :for="`inspector-search-${pluginId}`">{{ t('inspector.search') }}</label>
        <input
          :id="`inspector-search-${pluginId}`"
          v-model="search"
          type="search"
          :placeholder="t('inspector.searchPlaceholder')"
          @input="searchChanged"
        >
      </form>
      <p
        v-if="loading"
        role="status"
      >
        {{ t('inspector.loading') }}
      </p>
      <div
        v-if="page"
        class="inspector-workbench"
        :class="{'has-detail': !!detail}"
      >
        <div class="inspector-inventory">
          <p class="setting-hint">
            {{ t('inspector.pageCount', { first: page.rows.length ? page.query.offset + 1 : 0, last: page.query.offset + page.rows.length, matched: page.matched_rows }) }} · {{ t('inspector.total', { total: page.total_rows }) }}
          </p>
          <div
            v-if="page.rows.length"
            class="inspector-table-scroll"
            tabindex="0"
            role="region"
            :aria-label="t(`inspector.${section}`)"
          >
            <table>
              <thead>
                <tr>
                  <th>{{ t('inspector.item') }}</th><th>{{ t('inspector.source') }}</th><th v-if="section === 'changes'">
                    {{ t('inspector.before') }}
                  </th><th>{{ section === 'changes' ? t('inspector.after') : t('inspector.value') }}</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="row in page.rows"
                  :key="row.id"
                  :class="{'inspector-selected-row': detail?.row.id === row.id}"
                >
                  <td>
                    <button
                      class="text-button inspector-row-inspect"
                      :aria-label="t('inspector.inspectRow', {path: row.path.text})"
                      @click="readDetail(row, row.after ? 'after' : row.before ? 'before' : 'path')"
                    >
                      {{ t('inspector.inspect') }}
                    </button>
                    <InspectorCell
                      :cell="row.path"
                      :label="t('inspector.detailFor', { path: row.path.text, field: t('inspector.item') })"
                      @details="readDetail(row, 'path')"
                    />
                  </td>
                  <td>
                    {{ t(`inspector.${row.source}`) }}<template v-if="row.change">
                      · {{ t(`inspector.${row.change}`) }}
                    </template>
                  </td>
                  <td v-if="section === 'changes'">
                    <InspectorCell
                      :cell="row.before"
                      :label="t('inspector.detailFor', { path: row.path.text, field: t('inspector.before') })"
                      @details="readDetail(row, 'before')"
                    />
                  </td>
                  <td>
                    <InspectorCell
                      :cell="section === 'changes' ? row.after : row.after ?? row.before"
                      :label="t('inspector.detailFor', { path: row.path.text, field: t('inspector.value') })"
                      @details="readDetail(row, row.after ? 'after' : 'before')"
                    />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <p
            v-else
            class="content-empty"
          >
            {{ t(page.total_rows === 0 ? 'inspector.emptyDataset' : 'inspector.empty') }}
          </p>
          <div class="inspector-actions inspector-pagination">
            <button
              class="text-button"
              :disabled="page.query.offset === 0"
              @click="paginate(Math.max(0, page!.query.offset - page!.query.limit))"
            >
              {{ t('inspector.previous') }}
            </button>
            <button
              class="text-button"
              :disabled="page.next_offset === null"
              @click="paginate(page!.next_offset!)"
            >
              {{ t('inspector.next') }}
            </button>
          </div>
          <ReportResults
            v-if="findings?.findings.length"
            :report="findings"
          />
        </div>
        <section
          v-if="detail"
          class="inspector-detail"
          :aria-busy="detailLoading"
        >
          <div class="inspector-actions">
            <h3>{{ t('inspector.detail') }}</h3><button
              class="text-button"
              @click="closeDetail"
            >
              {{ t('inspector.close') }}
            </button>
          </div>
          <p class="inspector-detail-path">
            {{ detail.row.path.text }}
          </p>
          <div
            class="inspector-actions"
            role="group"
            :aria-label="t('inspector.detailField')"
          >
            <button
              v-for="field in (['path','before','after'] as InspectorField[]).filter(field => detail!.row[field])"
              :key="field"
              class="text-button"
              :aria-pressed="detail.field === field"
              :disabled="detailLoading"
              @click="readDetail(detail.row, field)"
            >
              {{ t(field === 'path' ? 'inspector.item' : field === 'before' ? 'inspector.before' : 'inspector.after') }}
            </button>
          </div>
          <pre>{{ detail.text }}</pre>
          <p class="setting-hint">
            {{ t('inspector.detailCount', { loaded: detailBytes, total: detail.total }) }}
          </p>
          <button
            v-if="detail.next !== null && !detailError"
            class="text-button"
            :disabled="detailLoading"
            @click="readDetail(detail.row, detail.field, detail.next)"
          >
            {{ detailLoading ? t('inspector.loading') : t('inspector.more') }}
          </button>
          <div
            v-if="detailError"
            role="alert"
            class="settings-error"
          >
            <p>{{ detailError }}</p><button
              class="text-button"
              @click="readDetail(detail!.row, detail!.field, detailQuery!.offset)"
            >
              {{ t('inspector.retry') }}
            </button>
          </div>
        </section>
      </div>
    </template>
    <div
      v-if="error"
      role="alert"
      class="settings-error"
    >
      <p>{{ error }}</p><button
        v-if="session"
        class="text-button"
        :disabled="loading"
        @click="query"
      >
        {{ t('inspector.retry') }}
      </button>
    </div>
  </div>
</template>
<style scoped>
.inspector-panel{flex:0 0 100%;min-width:0;max-width:100%}.inspector-actions{display:flex;align-items:center;gap:12px;flex-wrap:wrap}.inspector-pack-name{font-size:13px;overflow-wrap:anywhere}.inspector-sections{margin:16px 0}.inspector-sections [aria-pressed=true]{font-weight:600;text-decoration:underline}.inspector-search{margin:14px 0}.inspector-table-scroll{max-width:100%;overflow:auto}table{width:100%;border-collapse:collapse;font-size:12px;table-layout:fixed}th,td{text-align:left;vertical-align:top;padding:10px;border-bottom:1px solid var(--line);overflow-wrap:anywhere}th:first-child{width:30%}.inspector-pagination{justify-content:flex-end;margin-top:12px}.inspector-detail{padding:16px 0;border-top:1px solid var(--line)}.inspector-detail h3{font-size:14px;margin:0}.inspector-detail pre{font-family:inherit;white-space:pre-wrap;overflow-wrap:anywhere;max-height:360px;overflow:auto}
</style>
<style scoped>
.inspector-workbench{display:grid;grid-template-columns:minmax(0,1fr);gap:var(--ui-space-4)}.inspector-inventory{min-width:0}.inspector-row-inspect{display:block;margin-bottom:var(--ui-space-2)}.inspector-selected-row{background:var(--hover)}.inspector-detail-path{font-size:var(--ui-text-label);color:var(--muted);overflow-wrap:anywhere}.inspector-detail [aria-pressed=true]{font-weight:600;text-decoration:underline}.inspector-actions button{min-height:var(--ui-control-height);border-radius:var(--ui-radius-md)}@media(min-width:1050px){.inspector-workbench.has-detail{grid-template-columns:minmax(0,1.4fr) minmax(260px,1fr)}.has-detail .inspector-detail{border-top:0;border-left:1px solid var(--line);padding:0 0 0 var(--ui-space-4)}}
</style>
