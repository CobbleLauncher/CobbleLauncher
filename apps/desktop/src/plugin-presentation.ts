// SPDX-License-Identifier: MIT
import type { AppIconName } from './ui/icons';

export function pluginFallbackIcon(manifest: { capabilities?: string[] }): AppIconName {
  const capabilities = manifest.capabilities ?? [];
  if (capabilities.includes('blueprint-provider')) return 'network';
  if (capabilities.includes('viewer')) return 'monitor';
  if (capabilities.includes('tool-form')) return 'compass';
  if (capabilities.includes('report')) return 'file-search';
  if (capabilities.includes('message-composer')) return 'pencil';
  return 'puzzle';
}
