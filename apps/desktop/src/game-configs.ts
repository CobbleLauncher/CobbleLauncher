// SPDX-License-Identifier: MIT
export type GameConfigFile = { id: string; name: string; size: number };
export type GameConfigBackup = { id: string; createdAt: number };
export type GameConfigDocument = { id: string; name: string; text: string; revision: string; backups: GameConfigBackup[] };
export const MAX_CONFIG_BYTES = 1024 * 1024;
export function configTextBytes(text: string) { return new TextEncoder().encode(text).byteLength; }
