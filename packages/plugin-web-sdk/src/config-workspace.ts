// SPDX-License-Identifier: MIT

export type ConfigWorkspaceFile = { id: string; name: string; size: number };
export type ConfigWorkspaceBackup = { id: string; createdAt: number };
export type ConfigWorkspaceDocument = { id: string; name: string; text: string; revision: string; backups: ConfigWorkspaceBackup[] };
export type ConfigWorkspaceBackupDocument = { id: string; text: string };
type ConfigWorkspaceRequest =
  | { method: 'configWorkspace.list'; params: { gameId: string } }
  | { method: 'configWorkspace.read'; params: { gameId: string; configId: string } }
  | { method: 'configWorkspace.readBackup'; params: { gameId: string; configId: string; backupId: string } }
  | { method: 'configWorkspace.write'; params: { gameId: string; configId: string; expectedRevision: string; text: string } }
  | { method: 'configWorkspace.restore'; params: { gameId: string; configId: string; backupId: string; expectedRevision: string } };
export type ConfigWorkspaceTransport = (request: ConfigWorkspaceRequest) => Promise<unknown>;

function gameId(value: string): string {
  if (!value || value.length > 128 || !/^[a-zA-Z0-9._-]+$/u.test(value)) throw new Error('INVALID_CONTEXT_ID');
  return value;
}
function configId(value: string): string {
  if (!/^[a-f0-9]{64}$/iu.test(value)) throw new Error('CONFIG_NOT_FOUND');
  return value;
}
function backupId(value: string): string {
  if (!/^\d{1,39}-[a-f0-9]{64}$/iu.test(value)) throw new Error('CONFIG_BACKUP_NOT_FOUND');
  return value;
}
function revision(value: string): string {
  if (!/^[a-f0-9]{64}$/iu.test(value)) throw new Error('CONFIG_CHANGED');
  return value;
}
function configText(value: string): string {
  if (typeof value !== 'string' || value.includes('\0') || new TextEncoder().encode(value).byteLength > 1024 * 1024) throw new Error('CONFIG_TOO_LARGE');
  return value;
}

/** Instance-bound managed config operations; paths and backup contents remain host-resolved. */
export function createConfigWorkspace(transport: ConfigWorkspaceTransport) {
  return Object.freeze({
    list: (id: string) => transport({ method: 'configWorkspace.list', params: { gameId: gameId(id) } }) as Promise<ConfigWorkspaceFile[]>,
    read: (id: string, fileId: string) => transport({ method: 'configWorkspace.read', params: { gameId: gameId(id), configId: configId(fileId) } }) as Promise<ConfigWorkspaceDocument>,
    readBackup: (id: string, fileId: string, historyId: string) => transport({ method: 'configWorkspace.readBackup', params: { gameId: gameId(id), configId: configId(fileId), backupId: backupId(historyId) } }) as Promise<ConfigWorkspaceBackupDocument>,
    write: (id: string, fileId: string, expectedRevision: string, text: string) => transport({ method: 'configWorkspace.write', params: { gameId: gameId(id), configId: configId(fileId), expectedRevision: revision(expectedRevision), text: configText(text) } }) as Promise<ConfigWorkspaceDocument>,
    restore: (id: string, fileId: string, historyId: string, expectedRevision: string) => transport({ method: 'configWorkspace.restore', params: { gameId: gameId(id), configId: configId(fileId), backupId: backupId(historyId), expectedRevision: revision(expectedRevision) } }) as Promise<ConfigWorkspaceDocument>,
  });
}
