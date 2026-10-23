<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { t } from './i18n';
const props = defineProps<{ table: { columns: string[]; rows: string[][] }; index: number }>();
const pageSize = 50;
const page = ref(0);
const query = ref('');
const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase();
  return needle ? props.table.rows.filter(row => row.some(cell => cell.toLocaleLowerCase().includes(needle))) : props.table.rows;
});
const rows = computed(() => filtered.value.slice(page.value * pageSize, (page.value + 1) * pageSize));
watch(query, () => { page.value = 0; });
watch(() => props.table, () => { page.value = 0; query.value = ''; });
</script>
<template>
  <section class="report-table">
    <div
      v-if="table.rows.length > pageSize"
      class="content-toolbar"
    >
      <input
        v-model="query"
        type="search"
        :aria-label="t('reports.searchTable', { index })"
        :placeholder="t('reports.searchRows')"
      >
    </div>
    <div
      class="report-table-scroll"
      tabindex="0"
      role="region"
      :aria-label="t('reports.table', { index })"
    >
      <table>
        <thead>
          <tr>
            <th
              v-for="(column, columnIndex) in table.columns"
              :key="columnIndex"
              scope="col"
            >
              {{ column }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(row, rowIndex) in rows"
            :key="rowIndex"
          >
            <td
              v-for="(cell, cellIndex) in row"
              :key="cellIndex"
            >
              {{ cell }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <div
      v-if="table.rows.length > pageSize"
      class="report-pagination"
    >
      <span role="status">{{ t('reports.rowRange', { first: filtered.length ? page * pageSize + 1 : 0, last: Math.min((page + 1) * pageSize, filtered.length), total: filtered.length }) }}</span>
      <button
        class="text-button"
        :disabled="page === 0"
        @click="page--"
      >
        {{ t('reports.previous') }}
      </button>
      <button
        class="text-button"
        :disabled="(page + 1) * pageSize >= filtered.length"
        @click="page++"
      >
        {{ t('reports.next') }}
      </button>
    </div>
  </section>
</template>
<style scoped>
.report-table{min-width:0;margin-top:16px}.report-table-scroll{max-width:100%;overflow:auto}table{border-collapse:collapse;min-width:100%;font-size:13px}th,td{padding:8px 12px;text-align:left;border-bottom:1px solid var(--line);white-space:pre-wrap;overflow-wrap:anywhere;min-width:100px}.report-pagination{display:flex;align-items:center;justify-content:flex-end;gap:16px;margin-top:10px;font-size:12px}.report-pagination>span{margin-right:auto;color:var(--muted)}
</style>
