// SPDX-License-Identifier: MIT
import { invoke } from '@tauri-apps/api/core';
import { pluginLocale, type ExtensionContext, type ExtensionContribution, type ResolvedCommand } from '../extensions/contributions';
import type { AppIconName } from '../ui/icons';

export interface CommandAction {
  id: string;
  label: string;
  keywords?: string[];
  icon?: AppIconName;
  shortcutId?: string;
  suggestedShortcut?: string;
  group?: string;
  pinned?: boolean;
  quickEntry?: boolean;
  recentAt?: number;
  contextPriority?: number;
  enabled?: boolean;
  run: () => void | Promise<void>;
}

interface SearchText {
  original: string;
  fullPinyin: string;
  initials: string;
}

interface SearchIndex {
  label: SearchText;
  keywords: SearchText[];
}

let searchIndex = new WeakMap<CommandAction, SearchIndex>();
let transliterate: typeof import('pinyin-pro')['pinyin'] | undefined;
let pinyinLoad: Promise<boolean> | undefined;
const hanzi = /\p{Script=Han}+/gu;

/** Load the dictionary only while the palette is in use; original text works immediately. */
export function preparePinyinSearch(): Promise<boolean> {
  pinyinLoad ??= import('pinyin-pro').then(module => {
    transliterate = module.pinyin;
    searchIndex = new WeakMap<CommandAction, SearchIndex>();
    return true;
  }).catch(() => { pinyinLoad = undefined; return false; });
  return pinyinLoad;
}

function indexedText(value: string): SearchText {
  const original = value.toLocaleLowerCase();
  if (!transliterate || !/\p{Script=Han}/u.test(value)) return { original, fullPinyin: '', initials: '' };
  const fullPinyin = original.replace(hanzi, run => transliterate!(run, { toneType: 'none' }).replaceAll(' ', ''));
  const initials = original.replace(hanzi, run => transliterate!(run, { pattern: 'first', toneType: 'none' }).replaceAll(' ', ''));
  return { original, fullPinyin, initials };
}

function indexFor(command: CommandAction): SearchIndex {
  let index = searchIndex.get(command);
  if (!index) {
    index = { label: indexedText(command.label), keywords: command.keywords?.map(indexedText) ?? [] };
    searchIndex.set(command, index);
  }
  return index;
}

function textScore(text: SearchText, word: string): number {
  if (text.original === word) return 1000;
  if (text.original.startsWith(word)) return 500;
  if (text.original.includes(word)) return 250;
  if (text.fullPinyin === word || text.initials === word) return 450;
  if (text.fullPinyin.startsWith(word) || text.initials.startsWith(word)) return 300;
  if (text.fullPinyin.includes(word) || text.initials.includes(word)) return 175;
  return -1;
}

export function searchCommands(commands: readonly CommandAction[], query: string): CommandAction[] {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  function score(command: CommandAction): number {
    let value = (command.pinned ? 10 : 0) + (command.contextPriority ?? 0);
    if (command.recentAt) value += Math.min(10, Math.max(0, 10 - (Date.now() - command.recentAt) / 86_400_000));
    if (!words.length) return value;
    const index = indexFor(command);
    for (const word of words) {
      const labelScore = textScore(index.label, word);
      if (labelScore >= 0) value += labelScore;
      else if (index.keywords.some(keyword => textScore(keyword, word) >= 0)) value += 100;
      else return -1;
    }
    return value;
  }
  return commands.filter(command => command.enabled !== false)
    .map((command, index) => ({ command, index, score: score(command) }))
    .filter(result => result.score >= 0)
    .sort((left, right) => right.score - left.score || left.index - right.index)
    .map(result => result.command);
}

/** Host actions are supplied by the same handlers used by navigation and context menus. */
export function commandRegistry(actions: readonly CommandAction[], extensionActions: readonly CommandAction[] = []): CommandAction[] {
  const seen = new Set<string>();
  return [...actions, ...extensionActions].filter(action => {
    if (!action.id || seen.has(action.id)) return false;
    seen.add(action.id);
    return true;
  });
}

export type ExtensionPosition = 'tools' | 'instance' | 'blueprint';

/** Native listing filters disabled plugins and context; native resolve checks again at use time. */
export async function listExtensionActions(
  position: ExtensionPosition,
  context: ExtensionContext | undefined,
  onResolved: (command: ResolvedCommand) => void | Promise<void>,
  isCurrent: () => boolean = () => true,
): Promise<CommandAction[]> {
  const request = { position, locale: pluginLocale(), ...(context ? { context: { ...context } } : {}) };
  const plugins = await invoke<ExtensionContribution[]>('list_extension_contributions', request);
  if (!isCurrent()) return [];
  return plugins.flatMap(plugin => plugin.commands
    .filter(command => command.menus.some(menu => menu.position === position))
    .map(command => ({
      id: `plugin:${plugin.pluginId}:${command.id}`,
      label: command.label,
      keywords: [plugin.pluginName, plugin.pluginId, command.id, ...(command.searchKeywords ?? [])],
      icon: (['puzzle', 'compass', 'settings', 'search', 'play', 'folder', 'info'].includes(command.icon ?? '') ? command.icon : 'puzzle') as AppIconName,
      quickEntry: command.quickEntry,
      shortcutId: `plugin:${plugin.pluginId}:${command.id}`,
      suggestedShortcut: command.suggestedShortcut,
      async run() {
        if (!isCurrent()) return;
        const resolved = await invoke<ResolvedCommand>('resolve_extension_command', {
          pluginId: plugin.pluginId, commandId: command.id, position,
          locale: pluginLocale(), ...(context ? { context: { ...context } } : {}),
        });
        if (isCurrent()) await onResolved(resolved);
      },
    })));
}
