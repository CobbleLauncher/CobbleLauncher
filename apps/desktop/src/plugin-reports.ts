// SPDX-License-Identifier: MIT
export type PluginReport = {
  schema_version: number;
  findings: {
    code: string;
    severity: 'info' | 'warning' | 'error';
    title: string;
    message: string;
    evidence: { input_index: number; first_line: number; last_line: number }[];
    suggestions: string[];
  }[];
  tables: { columns: string[]; rows: string[][] }[];
};

export function isReportPlugin(manifest: { api_version?: string; capabilities: string[] }): boolean {
  return ['0.1.0', '0.2.0'].includes(manifest.api_version ?? '')
    && manifest.capabilities.length === 1
    && manifest.capabilities[0] === 'report';
}

export function isInspectorPlugin(manifest: { api_version?: string; capabilities: string[]; file_extensions: string[]; report_protocols?: string[] }): boolean {
  return isReportPlugin(manifest) && (manifest.report_protocols ?? []).includes('cobble.mrpack-page@1')
    && manifest.file_extensions.some(extension => extension === 'json' || extension === 'mrpack');
}

export type PluginFolderReport = {report:PluginReport;files:number;skipped:number;truncated:boolean};
