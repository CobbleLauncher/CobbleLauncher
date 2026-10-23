// SPDX-License-Identifier: MIT
import { locale } from '../i18n';
export function pluginLocale() { return locale.value === 'zh-CN' ? 'zh-CN' : 'en'; }
export type ExtensionContext = { instanceId?: string; blueprintId?: string };
export type ContributionCommand = { id: string; label: string; workflow: 'report' | 'viewer'; menus: { position: 'tools' | 'instance' | 'blueprint'; when: string }[] };
export type ExtensionContribution = { pluginId: string; pluginName: string; commands: ContributionCommand[]; settings: { id: string; label: string; fields?: { scope: 'global' | 'instance' }[] }[] };
export type ResolvedCommand = { pluginId: string; commandId: string; label: string; workflow: 'report' | 'viewer'; instanceId?: string; blueprintId?: string };
