// SPDX-License-Identifier: MIT
import type { MarketKind } from './market';

export type FileInboxMarketLink = {
  provider: 'modrinth' | 'curseforge';
  kind: Exclude<MarketKind, 'blueprint'>;
  project: string;
  version?: string;
  gameId?: string;
};

const part = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9._-]{1,128}$/u.test(value);

export function parseFileInboxMarketEvent(value: unknown): FileInboxMarketLink | null {
  if (!value || typeof value !== 'object') return null;
  const item = value as Record<string, unknown>;
  if (item.provider !== 'modrinth' && item.provider !== 'curseforge') return null;
  if (!['mod', 'modpack', 'resource_pack', 'shader_pack'].includes(String(item.kind))) return null;
  if (item.provider === 'curseforge' && !['mod', 'modpack'].includes(String(item.kind))) return null;
  if (!part(item.project) || item.version != null && !part(item.version)) return null;
  if (item.gameId != null && !part(item.gameId)) return null;
  if (item.kind !== 'modpack' && !item.gameId) return null;
  if (item.provider === 'curseforge' && item.version != null && !/^\d+$/u.test(item.version as string)) return null;
  return {
    provider: item.provider,
    kind: item.kind as FileInboxMarketLink['kind'],
    project: item.project,
    ...(item.version == null ? {} : { version: item.version as string }),
    ...(item.gameId == null ? {} : { gameId: item.gameId as string }),
  };
}

export function findLinkedProject<T extends { slug: string; project_id: string }>(hits: T[], link: FileInboxMarketLink | undefined): T | undefined {
  if (!link) return undefined;
  return hits.find(hit => hit.project_id === link.project || hit.slug.toLowerCase() === link.project.toLowerCase());
}
export function linkedVersionId<T extends { version_id?: string; file_id?: string; version_number?: string }>(versions: T[], version: string | undefined): string {
  if (!version) return '';
  const match = versions.find(item => item.version_id === version || item.file_id === version || item.version_number === version);
  return match?.version_id ?? match?.file_id ?? '';
}
