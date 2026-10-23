// SPDX-License-Identifier: MIT
import { describe, expect, it } from 'vitest';
import { activeSection, extensionDetailPage, backPage, enterPage, goToParent, initialNavigation, returnToGameDetails, rootPage, toolPage } from './navigation';
import { BLUEPRINTS_TOOL_ID } from './tools';

describe('workspace page hierarchy', () => {
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

});
