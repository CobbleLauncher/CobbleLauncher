// SPDX-License-Identifier: MIT
import { onMounted, onUnmounted, watch, type Ref } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { parsePluginHandoffRequest, type PluginHandoffRequest } from './plugin-handoffs';
import { parseRecoveryNavigation, type RecoveryNavigationRequest } from './recovery-navigation';
import type { FileInboxResult, FileInboxHostRequest } from './file-inbox-host';
import { parseFileInboxMarketEvent, type FileInboxMarketLink } from './file-inbox-market';

type PackDropPreview = Extract<FileInboxResult, { kind: 'mrpack' | 'curseforge-pack' }>;
type Request = { kind: 'pack-preview'; value: PackDropPreview } | { kind: 'generated-blueprint'; blueprintId: string } | { kind: 'inbox' } | { kind: 'inbox-review'; value: FileInboxHostRequest } | { kind: 'inbox-market'; value: FileInboxMarketLink } | { kind: 'game'; gameId: string }
  | { kind: 'handoff'; value: PluginHandoffRequest } | { kind: 'recovery'; value: RecoveryNavigationRequest };
interface Options {
  native: boolean;
  loading: Ref<boolean>;
  busy: Ref<boolean>;
  packBlocked?: Ref<boolean>;
  hasGame: (id: string) => boolean;
  refreshGames: () => Promise<void>;
  confirmLeave: () => Promise<boolean>;
  hideTasks: () => void;
  openPackPreview?: (value: PackDropPreview) => void | Promise<void>;
  openInbox: () => void;
  openInboxReview: (request: FileInboxHostRequest) => Promise<void>;
  openInboxMarket: (request: FileInboxMarketLink) => void;
  openGame: (id: string) => void;
  openGeneratedBlueprint: (id: string) => void;
  openHandoff: (request: PluginHandoffRequest) => void;
  openRecovery: (request: RecoveryNavigationRequest) => void;
  error: (key: string) => void;
}

/** Main-window native events share one leave guard and never run a mutation. */
export function usePluginNavigation(options: Options) {
  const pending: Request[] = [], stops: UnlistenFn[] = [];
  let disposed = false, processing = false;
  async function drain() {
    if (disposed || processing || options.loading.value || options.busy.value) return;
    processing = true;
    try {
      while (pending.length && !disposed && !options.busy.value) {
        if (pending[0]?.kind === 'pack-preview' && options.packBlocked?.value) break;
        const request = pending.shift()!;
        const gameId = request.kind === 'game' ? request.gameId
          : request.kind === 'inbox-market' ? request.value.gameId ?? null
          : request.kind === 'recovery' ? request.value.game_id
            : request.kind === 'handoff' ? request.value.gameId : null;
        if (request.kind === 'game' && !options.hasGame(request.gameId)) await options.refreshGames();
        if (disposed) return;
        if (gameId !== null && !options.hasGame(gameId)) { options.error('shell.error.handoffGameUnavailable'); continue; }
        if (!await options.confirmLeave() || disposed) continue;
        if (options.busy.value || options.loading.value) { pending.unshift(request); break; }
        options.hideTasks();
        if (request.kind === 'generated-blueprint') options.openGeneratedBlueprint(request.blueprintId);
        else if (request.kind === 'inbox') options.openInbox();
        else if (request.kind === 'inbox-review') await options.openInboxReview(request.value);
        else if (request.kind === 'inbox-market') options.openInboxMarket(request.value);
        else if (request.kind === 'game') options.openGame(request.gameId);
        else if (request.kind === 'recovery') options.openRecovery(request.value);
        else if (request.kind === 'pack-preview') await options.openPackPreview?.(request.value);
        else options.openHandoff(request.value);
      }
    } catch { options.error('shell.error.windowAction'); }
    finally { processing = false; }
  }
  function enqueue(request: Request) {
    if (disposed) return;
    if (request.kind === 'inbox' && pending.some(item => item.kind === 'inbox')) return;
    pending.push(request);
    if (pending.length > 32) pending.shift();
    void drain();
  }
  async function subscribe(name: string, handler: (payload: unknown) => void) {
    const stop = await listen<unknown>(name, ({ payload }) => { if (!disposed) handler(payload); });
    if (disposed) stop(); else stops.push(stop);
  }
  watch([options.loading, options.busy, () => options.packBlocked?.value], () => { void drain(); });
  onMounted(() => {
    if (!options.native) return;
    const registrations = [
      subscribe('native-pack-preview', payload => {
        const value = payload as PackDropPreview | null;
        if (options.openPackPreview && value && ['mrpack', 'curseforge-pack'].includes(value.kind) && typeof value.preview?.preview_id === 'string') enqueue({ kind: 'pack-preview', value });
      }),
      subscribe('native-pack-preview-error', () => options.error('shell.error.packDropFailed')),
      subscribe('plugin-generated-blueprint-open', payload => {
        const item = payload as { blueprint_id?: unknown; plugin_id?: unknown } | null;
        if (typeof item?.blueprint_id === 'string' && /^[a-zA-Z0-9_-]{1,128}$/u.test(item.blueprint_id)
          && typeof item.plugin_id === 'string' && /^[a-z0-9][a-z0-9._-]{0,127}$/u.test(item.plugin_id)) enqueue({ kind: 'generated-blueprint', blueprintId: item.blueprint_id });
      }),
      subscribe('plugin-handoff-request', payload => { const value = parsePluginHandoffRequest(payload); if (value) enqueue({ kind: 'handoff', value }); }),
      subscribe('plugin-open-game-recovery', payload => { const value = parseRecoveryNavigation(payload); if (value) enqueue({ kind: 'recovery', value }); }),
      subscribe('plugin-game-open-request', payload => {
        const gameId = (payload as { gameId?: unknown } | null)?.gameId;
        if (typeof gameId === 'string' && /^[A-Za-z0-9._-]{1,128}$/u.test(gameId)) enqueue({ kind: 'game', gameId });
      }),
      subscribe('plugin-file-inbox-drop', payload => {
        const item = payload as { plugin_id?: unknown; count?: unknown } | null;
        if (item?.plugin_id === 'file-inbox' && typeof item.count === 'number' && Number.isInteger(item.count) && item.count > 0) enqueue({ kind: 'inbox' });
      }),
      subscribe('plugin-file-inbox-open', payload => {
        const item = payload as Partial<FileInboxHostRequest> | null;
        if (typeof item?.handoff_id === 'string' && /^[a-f0-9]{32}$/u.test(item.handoff_id)
          && typeof item.plugin_id === 'string' && /^[a-z0-9][a-z0-9._-]{0,127}$/u.test(item.plugin_id)) {
          enqueue({ kind: 'inbox-review', value: { handoff_id: item.handoff_id, plugin_id: item.plugin_id } });
        }
      }),
      subscribe('plugin-file-inbox-market', payload => {
        const value = parseFileInboxMarketEvent(payload);
        if (value) enqueue({ kind: 'inbox-market', value });
      }),
      subscribe('plugin-file-inbox-notice', payload => options.error(payload === 'unavailable' ? 'shell.error.fileInboxUnavailable' : 'shell.error.fileInboxFailed')),
    ];
    void Promise.allSettled(registrations).then(results => {
      if (!disposed && results.some(result => result.status === 'rejected')) options.error('shell.error.windowAction');
    });
  });
  onUnmounted(() => { disposed = true; pending.length = 0; for (const stop of stops) stop(); });
}
