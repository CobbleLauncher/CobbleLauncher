// SPDX-License-Identifier: MIT
export type PackUpdateContext = { supported: boolean; reason: null | 'not_managed_pack' | 'source_unavailable' | 'unsupported_format' | 'incomplete_pack' | 'archive_too_large'; provider: 'mrpack' | null; name: string | null; installed_version: string | null; online_versions: false; identity_relation: 'unknown' };
export type PackManifestFile = { identity: string; version_id: string | null; path: string; size_bytes: number; sha1: string | null; sha512: string | null };
export type PackFileChange = { before: PackManifestFile | null; after: PackManifestFile | null; local_state: 'clean' | 'modified' | 'missing' | 'unknown'; preserve: boolean };
export type PackUpdatePreview = { target_name: string; excluded_optional_files: number; observed_file_count: number; unobserved_file_count: number; current_version: string; target_version: string; identity_relation: 'unknown' | 'confirmed'; runtime_changed: boolean; added: PackFileChange[]; removed: PackFileChange[]; version_changed: PackFileChange[]; unchanged: PackFileChange[] };
export type PackUpdateGroup = 'conflicts' | 'version_changed' | 'added' | 'removed' | 'unchanged';
export const PACK_UPDATE_GROUPS: PackUpdateGroup[] = ['conflicts', 'version_changed', 'added', 'removed', 'unchanged'];
export function packUpdateGroups(preview: PackUpdatePreview): Record<PackUpdateGroup, PackFileChange[]> { return { ...preview, conflicts: [...preview.added, ...preview.version_changed, ...preview.removed].filter(change => change.local_state === 'modified' || change.local_state === 'unknown') }; }
export function packChangePath(change: PackFileChange) { return change.after?.path ?? change.before?.path ?? ''; }
export function packChangeMatches(change: PackFileChange, query: string) { const term = query.trim().toLocaleLowerCase(); return !term || [change.before?.path, change.after?.path, change.before?.version_id, change.after?.version_id].some(value => value?.toLocaleLowerCase().includes(term)); }
