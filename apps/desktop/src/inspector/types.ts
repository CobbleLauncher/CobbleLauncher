// SPDX-License-Identifier: MIT
import type { PluginReport } from '../plugin-reports';
export type CellPreview = { text: string; byte_length: number; complete: boolean };
export type InspectorSection = 'overview' | 'current' | 'baseline' | 'changes';
export type InspectorQuery = { section: InspectorSection; search: string; offset: number; limit: number };
export type InspectorField = 'path' | 'before' | 'after';
export type InspectorRow = { id: string; source: 'metadata' | 'dependency' | 'declared_file' | 'override_file' | 'comparison'; change: 'added' | 'removed' | 'changed' | 'unchanged' | null; path: CellPreview; before: CellPreview | null; after: CellPreview | null };
export type InspectorPage = { query: InspectorQuery; total_rows: number; matched_rows: number; next_offset: number | null; rows: InspectorRow[] };
export type PackSummary = { name: CellPreview; version_id: CellPreview; declared_files: number; override_files: number; declared_download_bytes: number };
export type InspectorSession = { sessionId: string; summary: { current: PackSummary; baseline: PackSummary | null; changes: { added: number; removed: number; changed: number; unchanged: number } | null } };
export type InspectorPageReply = { sessionId: string; revision: number; page: InspectorPage; report: PluginReport };
export type DetailQuery = { row_id: string; field: InspectorField; offset: number; limit: number };
export type DetailReply = { sessionId: string; revision: number; detail: { row_id: string; field: InspectorField; total_bytes: number; offset: number; next_offset: number | null; text: string } };
