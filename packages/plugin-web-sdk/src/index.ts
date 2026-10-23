// SPDX-License-Identifier: MIT
/** Narrow native-childview client. This SDK never exposes a raw native invoke. */
export type GameContext = { id: string; name: string; version: string };
export type Context = { locale: 'en' | 'zh-CN'; theme: 'light' | 'dark' | 'system'; instanceId: string | null; games: GameContext[]; keybindingTargetId?: string | null; keybindingGames?: GameContext[]; recentGames?: RecentGame[]; inspectionGames?: GameContext[] };
export type InspectionScope = 'mods'|'packs'|'worlds'|'backups'|'screenshots'|'storage'|'runtime'|'options'|'datapacks'|'servers';
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key:string]: JsonValue };
export type InspectionSnapshot = { schemaVersion:1;scope:InspectionScope;game:GameContext;data:JsonValue;revision:string };
export type RecentGame = GameContext & { lastLaunchedAt: number | null };
export type DiagnosticFile = { id: string; name: string; size: number; modifiedAt: number | null };
export type DiagnosticDocument = { id: string; name: string; text: string; revision: string };
export type ConfigFile = { id: string; name: string; size: number };
export type ConfigDocument = { id: string; name: string; text: string; revision: string };
export type Finding = { code: string; severity: 'info' | 'warning' | 'error'; title: string; message: string; evidence: {input_index: number; first_line: number; last_line: number}[]; suggestions: string[] };
export type Report = { schema_version: number; findings: Finding[]; tables: {columns: string[]; rows: string[][]}[] };
export type FolderReport = { report: Report; files: number; skipped: number; truncated: boolean };
export type KeybindingPreview = { sourceRevision: string; targetRevision: string; changes: { key: string; before: string; after: string }[]; skipped: number; compatible?: number; conflicts?: { key: string; otherKey: string; value: string }[]; skippedBindings?: { key: string; reason: 'missing-target' | 'unsupported-value' }[] };
export type KeybindingResult = { backupId: string; targetRevision: string };
export type FrontendMethod = 'context' | 'inspection.snapshot' | 'inspection.check' | 'diagnostics.list' | 'diagnostics.read' | 'diagnostics.check' | 'diagnostics.pick' | 'configs.list' | 'configs.read' | 'report.check' | 'report.cancel' | 'keybindings.preview' | 'keybindings.apply' | 'keybindings.restore';
export type Request = { method: FrontendMethod; params: Record<string, unknown> };
export type Transport = (request: Request) => Promise<unknown>;

function id(value: string): string {
  if (!value || value.length > 256 || /[\u0000-\u001f\u007f]/u.test(value)) throw new Error('INVALID_CONTEXT_ID');
  return value;
}

function inspectionScope(value:InspectionScope):InspectionScope {
 if(!['mods','packs','worlds','backups','screenshots','storage','runtime','options','datapacks','servers'].includes(value))throw new Error('INVALID_INSPECTION_SCOPE');
 return value;
}
export function createClient(transport: Transport) {
  return Object.freeze({
    context: () => transport({ method: 'context', params: {} }) as Promise<Context>,
    keybindings: Object.freeze({
      preview: (sourceGameId: string, targetGameId: string) => transport({ method: 'keybindings.preview', params: { sourceGameId: id(sourceGameId), targetGameId: id(targetGameId) } }) as Promise<KeybindingPreview>,
      apply: (sourceGameId: string, targetGameId: string, sourceRevision: string, targetRevision: string) => transport({ method: 'keybindings.apply', params: { sourceGameId: id(sourceGameId), targetGameId: id(targetGameId), sourceRevision: id(sourceRevision), targetRevision: id(targetRevision) } }) as Promise<KeybindingResult>,
      restore: (targetGameId: string, backupId: string, targetRevision: string) => transport({ method: 'keybindings.restore', params: { targetGameId: id(targetGameId), backupId: id(backupId), targetRevision: id(targetRevision) } }) as Promise<string>,
    }),
    inspection: Object.freeze({
      snapshot: (gameId:string,scope:InspectionScope) => transport({method:'inspection.snapshot',params:{gameId:id(gameId),scope:inspectionScope(scope)}}) as Promise<InspectionSnapshot>,
      check: (gameId:string,scope:InspectionScope,revision:string) => transport({method:'inspection.check',params:{gameId:id(gameId),scope:inspectionScope(scope),revision:id(revision)}}) as Promise<Report|null>,
    }),
    diagnostics: Object.freeze({
      list: (gameId: string) => transport({ method: 'diagnostics.list', params: { gameId: id(gameId) } }) as Promise<DiagnosticFile[]>,
      read: (gameId: string, diagnosticId: string) => transport({ method: 'diagnostics.read', params: { gameId: id(gameId), diagnosticId: id(diagnosticId) } }) as Promise<DiagnosticDocument>,
      check: (gameId: string, diagnosticId: string, revision: string) => transport({ method: 'diagnostics.check', params: { gameId: id(gameId), diagnosticId: id(diagnosticId), revision: id(revision) } }) as Promise<Report | null>,
      pick: () => transport({ method: 'diagnostics.pick', params: {} }) as Promise<Report | null>,
    }),
    configs: Object.freeze({
      list: (gameId: string) => transport({ method: 'configs.list', params: { gameId: id(gameId) } }) as Promise<ConfigFile[]>,
      read: (gameId: string, configId: string) => transport({ method: 'configs.read', params: { gameId: id(gameId), configId: id(configId) } }) as Promise<ConfigDocument>,
    }),
    report: Object.freeze({
      check: (gameId: string, configIds: readonly string[]) => {
        if (!configIds.length || configIds.length > 64 || new Set(configIds).size !== configIds.length) throw new Error('INVALID_CONFIG_SELECTION');
        return transport({ method: 'report.check', params: { gameId: id(gameId), configIds: configIds.map(id) } }) as Promise<FolderReport | null>;
      },
      cancel: () => transport({ method: 'report.cancel', params: {} }) as Promise<null>,
    }),
  });
}
export type Client = ReturnType<typeof createClient>;

/** Connect only to the source-bound bridge supplied by the native view shell. */
export function connect(): Client {
  const bridge = (globalThis as unknown as { __COBBLE_PLUGIN_BRIDGE__?: {request?: (method: string, params?: unknown) => Promise<unknown>} }).__COBBLE_PLUGIN_BRIDGE__;
  if (!bridge || typeof bridge.request !== 'function') throw new Error('PLUGIN_BRIDGE_UNAVAILABLE');
  return createClient(request => bridge.request!(request.method, request.params));
}
