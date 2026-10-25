// SPDX-License-Identifier: MIT
import { computed, onMounted, onUnmounted, ref, watch, type Ref } from 'vue';
import { afterHomePaint } from '../after-home-paint';
import type { Game } from '../games';
import { createGameActions } from '../game-actions';
import { locale } from '../i18n';
import type { AppPage, MainSection } from '../navigation';
import type { ResolvedCommand } from '../extensions/contributions';
import { commandRegistry, listExtensionActions, type CommandAction } from './registry';
import { readShortcuts, shortcutAction, shortcutChangedEvent } from './shortcuts';

const recentKey = 'cobble-command-recents-v1';
function readRecents(): Record<string, number> {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(recentKey) ?? '{}');
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) return Object.fromEntries(Object.entries(parsed).filter(([, value]) => typeof value === 'number' && Number.isFinite(value) && value > 0));
  } catch { /* Recents are optional. */ }
  return {};
}

export interface ShellCommandOptions {
  games: Ref<Game[]>;
  page: Ref<AppPage>;
  selectedGameId: Ref<string>;
  busy: Ref<boolean>;
  ready: Ref<boolean>;
  navigate: (section: MainSection) => void | Promise<void>;
  openGame: (id: string) => void | Promise<void>;
  launchGame: (id: string) => void | Promise<void>;
  searchGames: () => void | Promise<void>;
  back: () => void | Promise<void>;
  openSettings: () => void | Promise<void>;
  openExtension: (command: ResolvedCommand) => void | Promise<void>;
  onGameUpdated: (game: Game) => void;
  extraActions?: Ref<CommandAction[]>;
  onError?: (message: string) => void;
}

export function useShellCommands(options: ShellCommandOptions) {
  const paletteOpen = ref(false);
  const extensions = ref<CommandAction[]>([]);
  const extensionsLoading = ref(false);
  const shortcuts = ref(readShortcuts());
  const recent = ref(readRecents());
  let loadRevision = 0;
  let cancelScheduled: (() => void) | undefined;
  const selected = computed(() => options.games.value.find(game => game.id === options.selectedGameId.value));
  const labels = computed(() => locale.value === 'zh-CN'
    ? { library: '游戏库', packs: '整合包', tools: '工具', extensions: '扩展', settings: '设置', search: '搜索当前列表', back: '返回', launch: '启动游戏', pluginError: '无法加载扩展操作' }
    : { library: 'Games', packs: 'Modpacks', tools: 'Tools', extensions: 'Extensions', settings: 'Settings', search: 'Search current list', back: 'Back', launch: 'Launch game', pluginError: 'Could not load extension actions' });
  const host = computed<CommandAction[]>(() => [
    { id: 'navigation.library', label: labels.value.library, icon: 'folder', run: () => options.navigate('library') },
    { id: 'navigation.discover-packs', label: labels.value.packs, icon: 'compass', run: () => options.navigate('discover-packs') },
    { id: 'navigation.tools', label: labels.value.tools, icon: 'puzzle', run: () => options.navigate('tools') },
    { id: 'navigation.extensions', label: labels.value.extensions, icon: 'puzzle', run: () => options.navigate('extensions') },
    { id: 'settings.open', label: labels.value.settings, icon: 'settings', shortcutId: 'settings.open', run: options.openSettings },
    { id: 'library.search', label: labels.value.search, icon: 'search', shortcutId: 'library.search', run: options.searchGames },
    { id: 'navigation.back', label: labels.value.back, icon: 'arrow-left', shortcutId: 'navigation.back', enabled: options.page.value.view !== 'library', run: options.back },
    { id: 'game.launch', label: selected.value ? `${labels.value.launch}: ${selected.value.name}` : labels.value.launch, icon: 'play', shortcutId: 'game.launch', contextPriority: selected.value ? 20 : 0, enabled: !!selected.value && selected.value.status === 'installed' && (!selected.value.launch_state || selected.value.launch_state === 'idle') && !selected.value.content_installing && !options.busy.value, run: () => { if (selected.value) return options.launchGame(selected.value.id); } },
    ...options.games.value.filter(game => game.status === 'installed').map(game => ({ id: `game.open:${game.id}`, label: game.name, keywords: [labels.value.library], icon: 'folder' as const, recentAt: game.last_launched_at || undefined, contextPriority: game.id === selected.value?.id ? 20 : 0, pinned: game.pinned, run: () => options.openGame(game.id) })),
    ...(selected.value ? createGameActions(selected.value, { manage: id => { void options.openGame(id); }, updated: options.onGameUpdated }).filter(action => action.id !== 'game.manage').map(action => ({ ...action, contextPriority: 20 })) : []),
    ...(options.extraActions?.value ?? []),
  ]);
  function remember(id: string) {
    recent.value = Object.fromEntries(Object.entries({ ...recent.value, [id]: Date.now() }).sort((left, right) => right[1] - left[1]).slice(0, 50));
    try { localStorage.setItem(recentKey, JSON.stringify(recent.value)); } catch { /* Keep session recents. */ }
  }
  const commands = computed(() => commandRegistry(host.value, extensions.value).map(action => ({
    ...action,
    recentAt: recent.value[action.id] ?? action.recentAt,
    async run() { await action.run(); remember(action.id); },
  })));

  async function refreshExtensions() {
    const revision = ++loadRevision;
    extensionsLoading.value = true;
    const gameId = selected.value?.id;
    const current = () => revision === loadRevision && options.ready.value;
    try {
      const [tools, instance] = await Promise.all([
        listExtensionActions('tools', gameId ? { instanceId: gameId } : undefined, options.openExtension, current),
        gameId ? listExtensionActions('instance', { instanceId: gameId }, options.openExtension, current) : Promise.resolve([]),
      ]);
      if (current()) { extensions.value = commandRegistry(instance, tools); extensionsLoading.value = false; }
    } catch { if (current()) { extensionsLoading.value = false; options.onError?.(labels.value.pluginError); } }
  }
  function openPalette() { paletteOpen.value = true; extensions.value = []; if (options.ready.value) void refreshExtensions(); }
  function closePalette() { paletteOpen.value = false; }
  function handleKeydown(event: KeyboardEvent) {
    const modalOpen = !paletteOpen.value && !!document.querySelector('[role="dialog"][aria-modal="true"], [role="alertdialog"][aria-modal="true"]');
    const id = shortcutAction(event, shortcuts.value, { paletteOpen: paletteOpen.value, modalOpen });
    if (!id) return;
    if (id === 'palette.open') { event.preventDefault(); if (paletteOpen.value) closePalette(); else openPalette(); return; }
    const action = commands.value.find(command => (command.shortcutId ?? command.id) === id && command.enabled !== false);
    if (action) { event.preventDefault(); void Promise.resolve(action.run()).catch(() => options.onError?.(labels.value.pluginError)); }
  }
  function changed() { shortcuts.value = readShortcuts(); }
  function scheduleRefresh() {
    cancelScheduled?.();
    ++loadRevision;
    extensions.value = [];
    extensionsLoading.value = options.ready.value;
    if (!options.ready.value) return;
    cancelScheduled = afterHomePaint(() => { cancelScheduled = undefined; void refreshExtensions(); });
  }
  onMounted(() => { window.addEventListener('keydown', handleKeydown); window.addEventListener(shortcutChangedEvent, changed); scheduleRefresh(); });
  onUnmounted(() => { window.removeEventListener('keydown', handleKeydown); window.removeEventListener(shortcutChangedEvent, changed); cancelScheduled?.(); ++loadRevision; });
  watch([locale, options.selectedGameId, options.page, options.ready], scheduleRefresh);
  return { commands, paletteOpen, openPalette, closePalette, refreshExtensions, handleKeydown, shortcuts, extensionsLoading };
}
