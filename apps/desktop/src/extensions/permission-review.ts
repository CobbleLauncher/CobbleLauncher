// SPDX-License-Identifier: MIT
import type { PermissionNode } from '../plugin-permissions';

export type PermissionCategory = 'network' | 'files' | 'game' | 'messages' | 'interface' | 'storage' | 'clipboard' | 'exports' | 'handoff';
export type PermissionTone = 'neutral' | 'network' | 'files' | 'write';
export function permissionTone(node: PermissionNode): PermissionTone {
  if ('namespaces' in node) return 'network';
  if (node.permission.startsWith('launcher:ai/')) return 'network';
  if (['launcher:storage/cache/clear', 'launcher:instances/move-request', 'launcher:instances/copy-request'].includes(node.permission)) return 'write';
  if (node.permission === 'launcher:blueprints/generated-save') return 'write';
  if (node.permission === 'launcher:clipboard/write') return 'neutral';
  if (node.permission.endsWith('/write') || node.permission === 'launcher:world-snapshots/restore-request') return 'write';
  if ('origins' in node) return 'network';
  if ('extensions' in node || node.permission.startsWith('launcher:game-') || node.permission.startsWith('launcher:world-snapshots/') || node.permission.startsWith('launcher:worlds/') || node.permission.startsWith('launcher:images/')) return 'files';
  return 'neutral';
}
export function permissionCategory(node: PermissionNode): PermissionCategory {
  if ('namespaces' in node) return 'network';
  if (node.permission.startsWith('launcher:ai/')) return 'network';
  if (node.permission === 'launcher:files/import-request') return 'handoff';
  if (node.permission === 'launcher:play-manifest/export') return 'exports';
  if (node.permission === 'launcher:storage/cache/clear') return 'storage';
  if (node.permission.startsWith('launcher:instances/')) return 'game';
  if (node.permission === 'launcher:blueprints/generated-save') return 'exports';
  if (node.permission === 'launcher:exports/text') return 'exports';
  if (node.permission === 'launcher:clipboard/write') return 'clipboard';
  if (node.permission === 'launcher:storage/private') return 'storage';
  if ('origins' in node) return 'network';
  if ('extensions' in node || node.permission.startsWith('launcher:images/')) return 'files';
  if (node.permission.startsWith('launcher:game-') || node.permission.startsWith('launcher:world-snapshots/') || node.permission.startsWith('launcher:worlds/')) return 'game';
  if (node.permission.startsWith('launcher:composer/')) return 'messages';
  return 'interface';
}

/** Scopes that need an explicit acknowledgement separate from installation. */
export function requiresSensitiveConsent(node: PermissionNode): boolean {
  if ('namespaces' in node) return true;
  return ['launcher:ai/chat', 'launcher:ai/configure', 'launcher:provider/https/get', 'launcher:game-config/read',
    'launcher:game-config/write', 'launcher:storage/cache/clear', 'launcher:instances/move-request',
    'launcher:instances/copy-request', 'launcher:game-recovery/read', 'launcher:play-manifest/export',
    'launcher:resource/selected-folder/read', 'launcher:ui/webview',
    'launcher:game-keybindings/read', 'launcher:game-keybindings/write',
    'launcher:game-diagnostics/read','launcher:game-inspection/read',
    'launcher:world-snapshots/read','launcher:world-snapshots/restore-request',
    'launcher:worlds/seed/read'].includes(node.permission);
}

export function permissionConsentKey(node: PermissionNode): string {
  if ('namespaces' in node) return `${node.permission}:${[...node.namespaces].sort().join('|')}`;
  if ('scopes' in node) return `${node.permission}:${[...node.scopes].sort().join('|')}`;
  if ('origins' in node) return `${node.permission}:${[...node.origins].sort().join('|')}`;
  if ('extensions' in node) return `${node.permission}:${[...node.extensions].sort().join('|')}`;
  return node.permission;
}
