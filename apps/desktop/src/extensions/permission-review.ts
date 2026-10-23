// SPDX-License-Identifier: MIT
import type { PermissionNode } from '../plugin-permissions';

export type PermissionCategory = 'network' | 'files' | 'game' | 'messages' | 'interface';
export type PermissionTone = 'neutral' | 'network' | 'files' | 'write';
export function permissionTone(node: PermissionNode): PermissionTone {
  if (node.permission.endsWith('/write')) return 'write';
  if ('origins' in node) return 'network';
  if ('extensions' in node || node.permission.startsWith('launcher:game-') || node.permission.startsWith('launcher:images/')) return 'files';
  return 'neutral';
}
export function permissionCategory(node: PermissionNode): PermissionCategory {
  if ('origins' in node) return 'network';
  if ('extensions' in node || node.permission.startsWith('launcher:images/')) return 'files';
  if (node.permission.startsWith('launcher:game-')) return 'game';
  if (node.permission.startsWith('launcher:composer/')) return 'messages';
  return 'interface';
}

/** Scopes that need an explicit acknowledgement separate from installation. */
export function requiresSensitiveConsent(node: PermissionNode): boolean {
  return ['launcher:provider/https/get', 'launcher:game-config/read',
    'launcher:resource/selected-folder/read', 'launcher:ui/webview',
    'launcher:game-keybindings/read', 'launcher:game-keybindings/write',
    'launcher:game-diagnostics/read','launcher:game-inspection/read'].includes(node.permission);
}

export function permissionConsentKey(node: PermissionNode): string {
  if ('scopes' in node) return `${node.permission}:${[...node.scopes].sort().join('|')}`;
  if ('origins' in node) return `${node.permission}:${[...node.origins].sort().join('|')}`;
  if ('extensions' in node) return `${node.permission}:${[...node.extensions].sort().join('|')}`;
  return node.permission;
}
