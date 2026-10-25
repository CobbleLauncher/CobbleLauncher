// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { initialNavigation, enterPage, goToParent } from './navigation';
import { openPluginHandoff, parsePluginHandoffRequest } from './plugin-handoffs';

const request = { handoffId: 'a'.repeat(32), targetPluginId: 'coordinate-book', gameId: 'game-a' };

it('accepts only bounded native handoff event fields', () => {
  expect(parsePluginHandoffRequest(request)).toEqual(request);
  expect(parsePluginHandoffRequest({ ...request, handoffId: '../guess' })).toBeNull();
  expect(parsePluginHandoffRequest({ ...request, targetPluginId: 'C:\\plugin' })).toBeNull();
});

it('opens the receiver in the bound installed game and preserves its exact parent', () => {
  const sender = enterPage(initialNavigation(), { view: 'running', gameId: 'game-a' });
  const opened = openPluginHandoff(sender, request, id => id === 'game-a');
  expect(opened?.current).toMatchObject({ view: 'tools', location: { pluginId: 'coordinate-book', instanceId: 'game-a', handoffId: request.handoffId } });
  expect(goToParent(opened!).current).toEqual(sender.current);
  expect(openPluginHandoff(sender, request, () => false)).toBeNull();
});

 it('opens a general materials handoff without selecting a game', () => {
  const request=parsePluginHandoffRequest({handoffId:'a'.repeat(32),targetPluginId:'material-planner',gameId:null,interfaceId:'launcher:materials-import',version:'0.1.0'});
  expect(request).not.toBeNull();
  expect(parsePluginHandoffRequest({handoffId:'a'.repeat(32),targetPluginId:'coordinate-book',gameId:null})).toBeNull();
 });
