// SPDX-License-Identifier: MIT
export type ExportMod = { projectId: string; versionId: string; name: string; sha1: string; size: number };
export type ExportConfig = { id: string; name: string; size: number; text: string };
export type ExportPreview = { revision: string; mods: ExportMod[]; configs: ExportConfig[]; excludedMods: string[] };
export type ExportResult = { saved: boolean; mods: number; configs: number };
export type ShareDocument = {
  schemaVersion: 1; name: string; gameVersion: string; loader: string | null; loaderVersion: string | null;
  mods: { source: 'modrinth'; projectId: string; versionId: string; name: string; sha1: string; size: number }[];
  configs: { path: string; text: string; sha256: string }[];
};
export type ImportResult = { importId: string; document: ShareDocument };
export type ApplyPreview = {
  planId: string;
  changes: { projectId: string; name: string; fromVersion: string | null; toVersion: string; filename: string; enabled: boolean }[];
  skipped: { projectId: string; name: string; reason: 'alreadyInstalled' }[];
  configs: { path: string; currentText: string | null; incomingText: string; currentRevision: string | null; choice: 'keep' }[];
  additionalLocalMods: number;
};
export type ConfigChoice = { path: string; choice: 'keep' | 'adopt' };
export type ConfigBackup = { path: string; configId: string; backupId: string | null; newRevision: string; undoId: string | null };
export type ApplyResult = { updated: number; added: number; configsWritten: number; configBackups: ConfigBackup[]; operationId: string | null; configError: string | null; complete: boolean };
