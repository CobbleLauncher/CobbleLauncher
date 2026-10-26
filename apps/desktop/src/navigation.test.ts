// SPDX-License-Identifier: MIT
import { describe, expect, it } from 'vitest';
import { activeSection, extensionDetailPage, backPage, enterPage, goToParent, initialNavigation, returnToGameDetails, rootPage, toolPage } from './navigation';
import { BLUEPRINTS_TOOL_ID } from './tools';

describe('workspace page hierarchy', () => {
  it('keeps the storage wizard under settings with one return destination', () => {
    const settings = enterPage(initialNavigation(), { view: 'settings' });
    const moving = enterPage(settings, { view: 'move-games', taskId: 'move-1' });
    expect(activeSection(moving.current, moving.parents)).toBe('settings');
    expect(goToParent(moving).current).toEqual({ view: 'settings' });
    expect(enterPage(moving, { view: 'move-games', taskId: 'move-1' }).parents).toEqual(moving.parents);
  });
  it('returns game tools and invitations to the same running session', () => {
    const running = enterPage(initialNavigation(), { view: 'running', gameId: 'live-game' });
    const tool = toolPage(running, { pluginId: 'command-pocket', instanceId: 'live-game' });
    expect(goToParent(tool).current).toEqual({ view: 'running', gameId: 'live-game' });
    const mesh = enterPage(running, { view: 'mesh', gameId: 'live-game' });
    expect(goToParent(mesh).current).toEqual(running.current);
    const meshGroup = enterPage(running, { view: 'mesh-group', gameId: 'live-game' });
    expect(goToParent(meshGroup).current).toEqual(running.current);
    expect(activeSection(running.current, running.parents)).toBe('library');
  });
  it('clears child history on every explicit top-level destination', () => {
    const details = enterPage(initialNavigation(), { view: 'details', gameId: 'game-a' });
    const child = enterPage(details, { view: 'mesh', gameId: 'game-a' });
    for (const section of ['library', 'discover-packs', 'tools', 'extensions', 'mesh', 'settings'] as const) {
      const state = enterPage(child, rootPage(section), 'root');
      expect(backPage(state)).toBeUndefined();
      expect(goToParent(state)).toBe(state);
      expect(activeSection(state.current, state.parents)).toBe(section);
    }
  });

  it('returns imported packs through the exact game market and details context', () => {
    let state = enterPage(initialNavigation(), { view: 'details', gameId: 'game-a' });
    state = enterPage(state, { view: 'discover-packs', kind: 'modpack', gameId: 'game-a' });
    state = enterPage(state, { view: 'import' });
    expect(activeSection(state.current, state.parents)).toBe('discover-packs');
    state = goToParent(state);
    expect(state.current).toEqual({ view: 'discover-packs', kind: 'modpack', gameId: 'game-a' });
    state = goToParent(state);
    expect(state.current).toEqual({ view: 'details', gameId: 'game-a' });
    expect(goToParent(state).current.view).toBe('library');
  });

  it('keeps game exchange modes distinct and returns each route to the original game', () => {
    let state = enterPage(initialNavigation(), { view: 'details', gameId: 'game-a' });
    const details = state.current;
    state = enterPage(state, { view: 'share-changes', gameId: 'game-a', mode: 'export' });
    const exported = state.current;
    state = enterPage(state, { view: 'share-changes', gameId: 'game-a', mode: 'import' });
    expect(state.current).toEqual({ view: 'share-changes', gameId: 'game-a', mode: 'import' });
    expect(backPage(state)).toEqual(exported);
    state = goToParent(state);
    expect(state.current).toEqual(exported);
    state = goToParent(state);
    expect(state.current).toEqual(details);
    expect(state.current).toEqual({ view: 'details', gameId: 'game-a' });

    const transfer = enterPage(state, { view: 'game-transfer', gameId: 'game-a', taskId: 'task-17', offerId: 'offer-29' });
    expect(transfer.current).toEqual({ view: 'game-transfer', gameId: 'game-a', taskId: 'task-17', offerId: 'offer-29' });
    expect(goToParent(transfer).current).toEqual(details);
  });

  it('retains an exact provider project and version link while visiting an exchange route', () => {
    const market = {
      view: 'discover-packs' as const,
      kind: 'mod' as const,
      gameId: 'game-a',
      query: 'wooden tools',
      link: { provider: 'curseforge' as const, kind: 'mod' as const, project: '248632', version: '5130472', gameId: 'game-a' },
    };
    let state = enterPage(initialNavigation(), market, 'root');
    state = enterPage(state, { view: 'share-changes', gameId: 'game-a', mode: 'import' });
    expect(state.market).toEqual(market);
    state = goToParent(state);
    expect(state.current).toEqual(market);
    expect(state.market.link).toEqual({ provider: 'curseforge', kind: 'mod', project: '248632', version: '5130472', gameId: 'game-a' });
  });

  it('has one viewer parent and treats blueprint tabs and title updates as replacements', () => {
    let state = enterPage(initialNavigation(), { view: 'tools' }, 'root');
    state = toolPage(state, { pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'library' });
    state = toolPage(state, { pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'online' });
    expect(state.parents).toEqual([{ view: 'tools' }]);
    state = toolPage(state, { pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'viewer', title: 'Original' });
    state = toolPage(state, { pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'viewer', title: 'Loaded name' });
    expect(state.parents).toHaveLength(2);
    state = goToParent(state);
    expect(state.current).toMatchObject({ view: 'tools', location: { blueprintView: 'online' } });
    expect(goToParent(state).current).toEqual({ view: 'tools' });
  });

  it('does not retain a reopen command or duplicate a library frame when leaving a viewer', () => {
    let state = enterPage(initialNavigation(), { view: 'tools' }, 'root');
    state = toolPage(state, { pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'library', command: { pluginId: 'provider', commandId: 'open', label: 'Open', workflow: 'viewer', blueprintId: 'saved-a', instanceId: 'game-a' } });
    state = toolPage(state, { pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'viewer', title: 'Saved' });
    expect(backPage(state)).toMatchObject({ location: { instanceId: 'game-a', command: undefined } });
    state = toolPage(state, { pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'library', instanceId: 'game-a' });
    expect(state.parents).toEqual([{ view: 'tools' }]);
  });

  it('restores profile flows to their parent without losing the pending game', () => {
    let state = enterPage(initialNavigation(), { view: 'details', gameId: 'game-a' });
    state = enterPage(state, { view: 'profiles' });
    state = enterPage(state, { view: 'add-profile' });
    expect(goToParent(state).current.view).toBe('profiles');
    state = returnToGameDetails(state, 'game-a');
    expect(state.current).toEqual({ view: 'details', gameId: 'game-a' });
    expect(state.parents).toEqual([{ view: 'library' }]);
    const settings = enterPage(initialNavigation(), { view: 'settings' }, 'root');
    const profiles = enterPage(settings, { view: 'profiles' });
    expect(activeSection(profiles.current, profiles.parents)).toBe('settings');
    expect(goToParent(profiles).current.view).toBe('settings');
  });

  it('does not create repeated parent frames for opening the current page', () => {
    const once = enterPage(initialNavigation(), { view: 'profiles' });
    expect(enterPage(once, { view: 'profiles' }).parents).toEqual(once.parents);
  });
  it('returns extension details and recommendations to the exact source candidate',()=>{
    const candidate={sourceId:'subscribed-source',pluginId:'optional-provider'};
    let state=enterPage(initialNavigation(),{view:'extensions',candidate},'root');
    state=extensionDetailPage(state,'disabled-plugin','Disabled plugin');
    expect(activeSection(state.current,state.parents)).toBe('extensions');
    expect(backPage(state)).toEqual({view:'extensions',candidate});
    state=extensionDetailPage(state,'recommended-plugin','Recommended plugin');
    expect(state.parents).toHaveLength(1);
    expect(goToParent(state).current).toEqual({view:'extensions',candidate});
    expect(extensionDetailPage(initialNavigation(),'missing-plugin').parents).toEqual([{view:'extensions'}]);
  });

  it('refreshes a receiver handoff token without adding another page frame',()=>{
    const initial = toolPage(enterPage(initialNavigation(), {view:'running',gameId:'game-a'}), {pluginId:'coordinate-book',instanceId:'game-a',handoffId:'token-one'});
    const refreshed = toolPage(initial, {pluginId:'coordinate-book',instanceId:'game-a',handoffId:'token-two'});
    expect(refreshed.current).toMatchObject({view:'tools',location:{pluginId:'coordinate-book',instanceId:'game-a',handoffId:'token-two'}});
    expect(refreshed.parents).toEqual(initial.parents);
    expect(goToParent(refreshed).current).toEqual({view:'running',gameId:'game-a'});
  });

  it('does not retain a consumed handoff token in tool navigation history',()=>{
    const receiver = toolPage(enterPage(initialNavigation(), {view:'running',gameId:'game-a'}), {pluginId:'coordinate-book',instanceId:'game-a',handoffId:'token-one'});
    const otherTool = toolPage(receiver, {pluginId:'another-tool',instanceId:'game-a'});
    expect(receiver.current).toMatchObject({view:'tools',location:{handoffId:'token-one'}});
    expect(otherTool.current).toMatchObject({view:'tools',location:{pluginId:'another-tool'}});
    expect(otherTool.parents.at(-1)).toEqual({view:'tools',location:{pluginId:'coordinate-book',instanceId:'game-a'}});
    expect(goToParent(otherTool).current).toEqual({view:'tools',location:{pluginId:'coordinate-book',instanceId:'game-a'}});
  });

});
