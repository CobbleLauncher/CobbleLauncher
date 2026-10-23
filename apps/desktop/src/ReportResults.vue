<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import ReportTable from './ReportTable.vue';
import type { PluginReport } from './plugin-reports';
defineProps<{ report: PluginReport }>();
</script>
<template>
  <div
    class="report-results"
    aria-live="polite"
  >
    <p v-if="!report.findings.length">
      {{ t('reports.empty') }}
    </p>
    <article
      v-for="(finding, index) in report.findings"
      :key="index"
      class="report-finding"
    >
      <h3>
        <span
          class="report-severity"
          :data-severity="finding.severity"
        >{{ t(`reports.${finding.severity}`) }}</span> {{ finding.title }}
      </h3>
      <p class="report-text">
        {{ finding.message }}
      </p>
      <ul
        v-if="finding.evidence.length"
        class="report-evidence"
      >
        <li
          v-for="(evidence, evidenceIndex) in finding.evidence"
          :key="evidenceIndex"
        >
          {{ t('reports.evidence', { input: evidence.input_index + 1, first: evidence.first_line, last: evidence.last_line }) }}
        </li>
      </ul>
      <ul v-if="finding.suggestions.length">
        <li
          v-for="(suggestion, suggestionIndex) in finding.suggestions"
          :key="suggestionIndex"
          class="report-text"
        >
          {{ suggestion }}
        </li>
      </ul>
    </article>
    <ReportTable
      v-for="(table, tableIndex) in report.tables"
      :key="tableIndex"
      :table="table"
      :index="tableIndex + 1"
    />
  </div>
</template>
<style scoped>
.report-results{min-width:0}.report-finding{padding:12px 0;border-bottom:1px solid var(--line)}.report-finding h3{font-size:14px;margin:0}.report-text{white-space:pre-wrap;overflow-wrap:anywhere}.report-severity{font-size:12px;font-weight:normal}.report-severity[data-severity=error]{color:var(--danger, #ba3939)}.report-evidence{font-size:12px;color:var(--text-secondary)}
</style>
