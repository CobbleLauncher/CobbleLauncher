export { listServerContexts } from './server-context.ts';
export type * from './server-context.ts';
import { createGeneratedBlueprint } from './generated-blueprint.ts';
export type * from './generated-blueprint.ts';
// SPDX-License-Identifier: MIT
export { applyHostAppearance, hostContext } from './appearance.ts';
import type { LocalWorldMetadata, SeedMapCapabilities, SeedMapRequest, SeedSearchRequest, SeedMapResult, SeedSearchResult } from './seed-map';
import { createDataClient } from './data.ts';
import { createMeshEvents } from './mesh-events.ts';
export { createMeshEvents } from './mesh-events.ts';
export type * from './mesh-events.ts';
import { createPlayManifest } from './play-manifest.ts';
export type * from './play-manifest.ts';
import { createStorageWorkspace } from './storage-workspace.ts';
export type * from './storage-workspace.ts';
import { createConfigWorkspace } from './config-workspace.ts';
export type * from './config-workspace.ts';
import { createRecoveryWorkspace } from './recovery-workspace.ts';
export type * from './recovery-workspace.ts';
import { createFileInbox } from './file-inbox.ts';
export type * from './file-inbox.ts';
import { createLangPackage } from './lang-package.ts';
export type * from './lang-package.ts';
import { createTrialCopy } from './trial-copy.ts';
import { createCrashExperiments } from './crash-experiments.ts';
export type * from './crash-experiments.ts';
export type * from './trial-copy.ts';
import { createExports } from './exports.ts';
export { MATERIALS_IMPORT_INTERFACE, MATERIALS_IMPORT_VERSION } from './materials.ts';
export type { MaterialRequirement, MaterialsPayload, MaterialsHandoffDraft } from './materials.ts';
export type { TextExport } from './exports.ts';
export { redactLog } from './redaction.ts';
export type { LogRedaction, RedactionChange, RedactionKind } from './redaction.ts';
import { createHandoffs } from './handoffs.ts';
export { WAYPOINT_IMPORT_INTERFACE, WAYPOINT_IMPORT_VERSION } from './handoffs.ts';
export type { WaypointPayload, WaypointDimension, HandoffReceiver, WaypointHandoffDraft } from './handoffs';
export type { DataScope, DataDocument, DataTarget, DataWrite, DataRemove } from './data';
export type * from './seed-map';
function seedRequestId(value: string): string {
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(value)) throw new Error('SEED_MAP_REQUEST_INVALID');
  return value;
}
function seedInput<T extends { seed: string }>(request: T): T {
  if (typeof request.seed !== 'string') throw new Error('SEED_MAP_INVALID_SEED');
  return request;
}
/** Narrow native-childview client. This SDK never exposes a raw native invoke. */
export type GameContext = { id: string; name: string; version: string };
export type Context = { locale: 'en' | 'zh-CN'; theme: 'light' | 'dark' | 'system'; appearance?: { accent: 'green' | 'blue' | 'amber' | 'purple'; density: 'standard' | 'compact' | 'comfortable'; scale: number; textSize?: number; motion: 'system' | 'reduced' } | null; configRestoreTarget?: {configId:string;backupId:string}|null; instanceId: string | null; games: GameContext[]; keybindingTargetId?: string | null; keybindingGames?: GameContext[]; recentGames?: RecentGame[]; inspectionGames?: GameContext[] };
export type InspectionScope = 'mods'|'packs'|'worlds'|'backups'|'screenshots'|'storage'|'runtime'|'options'|'datapacks'|'servers';
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key:string]: JsonValue };
export type InspectionSnapshot = { schemaVersion:1;scope:InspectionScope;game:GameContext;data:JsonValue;revision:string };
export type ModInventory = { available: boolean; truncated: boolean; items: { id: string; name: string; version: string | null; loader: string | null; disabled: boolean; declaredIds: string[]; dependencies: { id: string; version: string | null; optional: boolean }[] }[] };
export type WorldSnapshot = { id: string; world_id: string; name: string; created_at: string; processed_bytes: number; added_bytes: number; protected: boolean; automatic: boolean };
export type WorldSnapshotChange = { path: string; kind: 'added' | 'modified' | 'deleted'; before_bytes: number | null; after_bytes: number | null };
/** Summary counts cover all changed files; total and rows reflect the optional path query. */
export type WorldSnapshotComparison = { added: number; modified: number; deleted: number; unchanged: number; total: number; offset: number; rows: WorldSnapshotChange[] };
export type RecentGame = GameContext & { lastLaunchedAt: number | null };
export type DiagnosticFile = { id: string; name: string; size: number; modifiedAt: number | null };
export type DiagnosticDocument = { id: string; name: string; text: string; revision: string };
export type AiStatus = { configured: boolean; endpoint: string | null; model: string | null };
export type AiMessage = { role: 'user' | 'assistant'; content: string };
export type AiDiagnostic = { gameId: string; diagnosticId: string; revision: string };
export type ConfigFile = { id: string; name: string; size: number };
export type ConfigDocument = { id: string; name: string; text: string; revision: string };
export type Finding = { code: string; severity: 'info' | 'warning' | 'error'; title: string; message: string; evidence: {input_index: number; first_line: number; last_line: number}[]; suggestions: string[] };
export type Report = { schema_version: number; findings: Finding[]; tables: {columns: string[]; rows: string[][]}[] };
export type FolderReport = { report: Report; files: number; skipped: number; truncated: boolean };
export type KeybindingPreview = { sourceRevision: string; targetRevision: string; changes: { key: string; before: string; after: string }[]; skipped: number; compatible?: number; conflicts?: { key: string; otherKey: string; value: string }[]; skippedBindings?: { key: string; reason: 'missing-target' | 'unsupported-value' }[] };
export type KeybindingResult = { backupId: string; targetRevision: string };
export type FrontendMethod = 'mesh.peers' | 'mesh.subscribe' | 'mesh.receive' | 'mesh.unsubscribe' | 'mesh.send' | 'recovery.experimentPreview' | 'recovery.experimentOpen' | 'trialCopy.cleanup' | 'generatedBlueprint.save' | 'generatedBlueprint.open' | 'ai.status' | 'ai.configure' | 'ai.clear' | 'ai.chat' | 'ai.cancel' | 'recovery.snapshot' | 'recovery.openRequest' | 'fileInbox.pick' | 'fileInbox.pending' | 'fileInbox.open' | 'fileInbox.release' | 'langPackage.pick' | 'langPackage.check' | 'langPackage.document' | 'langPackage.cancel' | 'trialCopy.preview' | 'trialCopy.pickDestination' | 'trialCopy.start' | 'trialCopy.task' | 'trialCopy.cancel' | 'trialCopy.openGame' | 'playManifest.preview' | 'playManifest.export' | 'storageWorkspace.openSource' | 'storageWorkspace.overview' | 'storageWorkspace.pickDestination' | 'storageWorkspace.cleanCache' | 'storageWorkspace.startMove' | 'storageWorkspace.task' | 'storageWorkspace.cancel' | 'storageWorkspace.releaseSource' | 'configWorkspace.list' | 'configWorkspace.read' | 'configWorkspace.readBackup' | 'configWorkspace.write' | 'configWorkspace.restore' | 'exports.text' | 'clipboard.writeText' | 'data.read' | 'data.write' | 'data.remove' | 'view.setDirty' | 'handoffs.receivers' | 'handoffs.send' | 'handoffs.take' | 'worlds.metadata' | 'seedMap.capabilities' | 'seedMap.generate' | 'seedMap.search' | 'seedMap.cancel' | 'context' | 'inspection.snapshot' | 'inspection.check' | 'diagnostics.list' | 'diagnostics.read' | 'diagnostics.check' | 'diagnostics.pick' | 'configs.list' | 'configs.read' | 'report.pick' | 'report.check' | 'report.cancel' | 'keybindings.preview' | 'keybindings.apply' | 'keybindings.restore' | 'worldSnapshots.list' | 'worldSnapshots.compare' | 'worldSnapshots.openRestore';
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
function snapshotPage(offset:number,limit:number,query:string):void {
 if(!Number.isInteger(offset)||offset<0||offset>200_000||!Number.isInteger(limit)||limit<1||limit>100||[...query].length>120||/[\u0000-\u001f\u007f]/u.test(query))throw new Error('INVALID_SNAPSHOT_PAGE');
}
export function createClient(transport: Transport) {
  return Object.freeze({
    generatedBlueprint: createGeneratedBlueprint(transport),
    mesh: createMeshEvents(transport),
    ai: Object.freeze({
      status: () => transport({ method: 'ai.status', params: {} }) as Promise<AiStatus>,
      configure: (args: { endpoint: string; model: string; key: string }) => transport({ method: 'ai.configure', params: args }) as Promise<AiStatus>,
      clear: () => transport({ method: 'ai.clear', params: {} }) as Promise<AiStatus>,
      chat: (args: { requestId: string; messages: AiMessage[]; diagnostic?: AiDiagnostic }) => transport({ method: 'ai.chat', params: args }) as Promise<{ reply: string }>,
      cancel: (args: { requestId: string }) => transport({ method: 'ai.cancel', params: args }) as Promise<{ cancelled: boolean }>,
    }),
    playManifest: createPlayManifest(transport),
    storageWorkspace: createStorageWorkspace(transport),
    configWorkspace: createConfigWorkspace(transport),
    recovery: createRecoveryWorkspace(transport),
    fileInbox: createFileInbox(transport),
    langPackage: createLangPackage(transport),
    trialCopy: createTrialCopy(transport),
    crashExperiments: createCrashExperiments(transport),
    data: createDataClient(transport),
    exports: createExports(transport),
    handoffs: createHandoffs(transport),
    clipboard: Object.freeze({
      /** Copies user-requested text through the host. Clipboard reads are not exposed. */
      writeText: (text: string) => {
        if (typeof text !== 'string' || text.includes('\0') || new TextEncoder().encode(text).length > 8192) throw new Error('PLUGIN_CLIPBOARD_TEXT_INVALID');
        return transport({ method: 'clipboard.writeText', params: { text } }) as Promise<null>;
      },
    }),
    view: Object.freeze({
      /** Keep unsaved plugin edits behind the launcher's normal leave confirmation. */
      setDirty: (args: { dirty: boolean }) => {
        if (typeof args.dirty !== 'boolean') throw new Error('PLUGIN_VIEW_DIRTY_INVALID');
        return transport({ method: 'view.setDirty', params: { dirty: args.dirty } }) as Promise<null>;
      },
    }),
    worlds: Object.freeze({
      metadata: (gameId: string, worldId: string) => {
        if (!worldId || worldId.length > 128 || /[/\\\u0000-\u001f\u007f]/u.test(worldId) || ['.', '..'].includes(worldId)) throw new Error('PLUGIN_WORLD_SCOPE');
        return transport({ method: 'worlds.metadata', params: { gameId: id(gameId), worldId } }) as Promise<LocalWorldMetadata>;
      },
    }),
    seedMap: Object.freeze({
      capabilities: () => transport({ method: 'seedMap.capabilities', params: {} }) as Promise<SeedMapCapabilities>,
      generate: (args: { requestId: string; map: SeedMapRequest }) => transport({ method: 'seedMap.generate', params: { requestId: seedRequestId(args.requestId), map: seedInput(args.map) } }) as Promise<SeedMapResult>,
      search: (args: { requestId: string; search: SeedSearchRequest }) => transport({ method: 'seedMap.search', params: { requestId: seedRequestId(args.requestId), search: seedInput(args.search) } }) as Promise<SeedSearchResult>,
      /** Resolves after the matching worker has released its session capacity. */
      cancel: (args: { requestId: string }) => transport({ method: 'seedMap.cancel', params: { requestId: seedRequestId(args.requestId) } }) as Promise<boolean>,
    }),
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
    worldSnapshots: Object.freeze({
      list: (gameId:string,worldId:string) => transport({method:'worldSnapshots.list',params:{gameId:id(gameId),worldId:id(worldId)}}) as Promise<WorldSnapshot[]>,
      compare: (gameId:string,worldId:string,beforeId:string,afterId:string,offset:number,limit:number,query='') => {
        snapshotPage(offset,limit,query);
        return transport({method:'worldSnapshots.compare',params:{gameId:id(gameId),worldId:id(worldId),beforeId:id(beforeId),afterId:id(afterId),offset,limit,query}}) as Promise<WorldSnapshotComparison>;
      },
      /** Requests the launcher's confirmation flow; opening and restoration are not guaranteed. */
      openRestore: (gameId:string,worldId:string,snapshotId:string) => transport({method:'worldSnapshots.openRestore',params:{gameId:id(gameId),worldId:id(worldId),snapshotId:id(snapshotId)}}) as Promise<{requested:true}>,
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
      pick: () => transport({ method: 'report.pick', params: {} }) as Promise<Report | null>,
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
