// SPDX-License-Identifier: MIT
import type { GameAction } from './games';
import type { MarketKind } from './market';
import type { FileInboxMarketLink } from './file-inbox-market';
import type { ResolvedCommand } from './extensions/contributions';
import type { SnapshotRestoreTarget } from './world-snapshots';
import type { ConfigRestoreTarget, RecoveryDetailTarget } from './recovery-navigation';

export type MainSection = 'library' | 'discover-packs' | 'tools' | 'extensions' | 'mesh' | 'settings';
export type BlueprintView = 'library' | 'online' | 'viewer';
export interface ToolLocation {
  pluginId: string;
  command?: ResolvedCommand;
  instanceId?: string;
  keybindingTargetId?: string;
  configRestoreTarget?: ConfigRestoreTarget;
  handoffId?: string;
  blueprintView?: BlueprintView;
  blueprintId?: string;
  title?: string;
}
export interface ExtensionCandidate { sourceId: string; pluginId: string }
export type MarketPage = { view: 'discover-packs'; kind: MarketKind; gameId: string; query?: string; link?: FileInboxMarketLink };
export type AppPage =
  | { view: 'first-run' | 'library' | 'create' | 'import' | 'cf-import' | 'import-launcher' | 'settings' | 'profiles' | 'add-profile' }
  | { view: 'extensions'; candidate?: ExtensionCandidate }
  | { view: 'move-games'; taskId?: string }
  | { view: 'extension-detail'; pluginId: string; title?: string }
  | { view: 'details'; gameId: string; snapshotRestore?: SnapshotRestoreTarget; recoveryTarget?: RecoveryDetailTarget }
  | { view: 'running'; gameId: string }
  | { view: 'readiness'; gameId: string }
  | { view: 'mesh-group'; gameId: string; invitation?: string }
  | { view: 'game-action'; gameId: string; action: GameAction }
  | { view: 'share-changes'; gameId: string; mode: 'export' | 'import' }
  | { view: 'game-transfer'; gameId: string; taskId?: string; offerId?: string }
  | { view: 'mesh'; gameId: string; initialMode?: 'host' | 'join' | 'contacts'; initialContactId?: string; contactRequestId?: number }
  | MarketPage
  | { view: 'tools'; location?: ToolLocation };

export interface NavigationState {
  current: AppPage;
  parents: AppPage[];
  /** Retained input for mounted Market/Mesh views, never an independent route. */
  market: MarketPage;
  lastGameId: string;
}
export function initialNavigation(): NavigationState {
  return { current: { view: 'library' }, parents: [], market: { view: 'discover-packs', kind: 'modpack', gameId: '' }, lastGameId: '' };
}
export function rootPage(section: MainSection): AppPage {
  if (section === 'mesh') return { view: 'mesh', gameId: '' };
  return section === 'discover-packs' ? { view: section, kind: 'modpack', gameId: '' } : { view: section };
}
export function enterPage(state: NavigationState, page: AppPage, mode: 'push' | 'replace' | 'root' = 'push'): NavigationState {
  if(mode==='push' && pageKey(state.current)===pageKey(page)) mode='replace';
  let parent = state.current;
  if (parent.view === 'tools' && (parent.location?.handoffId || parent.location?.configRestoreTarget)) {
    const { handoffId: _handoffId, configRestoreTarget: _restoreTarget, ...location } = parent.location;
    void _handoffId; void _restoreTarget;
    parent = { ...parent, location };
  }
  if (parent.view === 'details' && parent.recoveryTarget) {
    const { recoveryTarget: _recoveryTarget, ...location } = parent;
    void _recoveryTarget; parent = location;
  }
  return {
    current: page,
    parents: mode === 'root' ? [] : mode === 'replace' ? state.parents : [...state.parents, parent],
    market: page.view === 'discover-packs' ? page : state.market,
    lastGameId: 'gameId' in page && page.gameId ? page.gameId : state.lastGameId,
  };
}
export function backPage(state: NavigationState): AppPage | undefined {
  return state.parents.at(-1);
}
export function goToParent(state: NavigationState): NavigationState {
  const parent = backPage(state);
  if (!parent) return state;
  return { ...enterPage(state, parent, 'replace'), parents: state.parents.slice(0, -1) };
}
export function returnToGameDetails(state:NavigationState,gameId:string):NavigationState {
  let index=state.parents.length-1;
  while(index>=0) {const parent=state.parents[index]!;if(parent.view==='details' && parent.gameId===gameId) break;index--;}
  return index>=0 ? {...state,current:{view:'details',gameId},parents:state.parents.slice(0,index),lastGameId:gameId}
    : enterPage(enterPage(state,{view:'library'},'root'),{view:'details',gameId});
}
export function activeSection(page: AppPage, parents:AppPage[]=[]): MainSection {
  if (page.view === 'move-games') return 'settings';
  if (['settings', 'extensions', 'tools', 'discover-packs', 'mesh'].includes(page.view)) return page.view as MainSection;
  for(let index=parents.length-1;index>=0;index--) {
    const parent=parents[index]!;
    if(['library','settings','extensions','tools','discover-packs','mesh'].includes(parent.view)) return parent.view as MainSection;
  }
  return 'library';
}
export function extensionDetailPage(state:NavigationState,pluginId:string,title?:string):NavigationState {
  if(state.current.view==='extension-detail') return enterPage(state,{view:'extension-detail',pluginId,title},'replace');
  const parent=state.current.view==='extensions' ? state : enterPage(state,{view:'extensions'},'root');
  return enterPage(parent,{view:'extension-detail',pluginId,title});
}
export function toolPage(state: NavigationState, location: ToolLocation): NavigationState {
  const current = state.current;
  if (current.view === 'tools' && current.location?.pluginId === location.pluginId) {
    const previous = current.location;
    if(previous.blueprintView==='viewer' && location.blueprintView && location.blueprintView!=='viewer') {
      const parent=backPage(state);
      const next=parent?.view==='tools' && parent.location?.pluginId===location.pluginId ? goToParent(state) : state;
      return enterPage(next,{view:'tools',location},'replace');
    }
    // Tabs and asynchronously supplied viewer titles replace their frame.
    const viewerOpened = location.blueprintView === 'viewer' && previous.blueprintView !== 'viewer';
    const parentState=viewerOpened ? enterPage(state,{view:'tools',location:{...previous,title:undefined,command:undefined,instanceId:previous.instanceId ?? previous.command?.instanceId}},'replace') : state;
    return enterPage(parentState, { view: 'tools', location }, viewerOpened ? 'push' : 'replace');
  }
  return enterPage(state, { view: 'tools', location });
}
export function pageKey(page:AppPage):string {
  if(page.view==='share-changes') return JSON.stringify([page.view,page.gameId,page.mode]);
  if(page.view==='game-transfer') return JSON.stringify([page.view,page.gameId,page.taskId,page.offerId]);
  if(page.view==='move-games') return JSON.stringify([page.view,page.taskId]);
  if(page.view==='tools') {const location=page.location;return JSON.stringify([page.view,location?.pluginId,location?.blueprintView,location?.instanceId,location?.keybindingTargetId,location?.handoffId,location?.configRestoreTarget,location?.command]);}
  if(page.view==='extension-detail') return JSON.stringify([page.view,page.pluginId]);
  if(page.view==='game-action') return JSON.stringify([page.view,page.gameId,page.action]);
  if(page.view==='discover-packs') return JSON.stringify([page.view,page.gameId,page.kind,page.query,page.link]);
  return JSON.stringify([page.view,'gameId' in page?page.gameId:null]);
}

