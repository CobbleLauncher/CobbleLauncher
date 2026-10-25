// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Location } from './model.ts';
import { portalCandidates, portalOriginLocations } from './portal-planner.ts';

const places: Location[] = [
  { id: 'home', name: 'Home', dimension: 'overworld', x: -17, y: 70, z: 40, note: '' },
  { id: 'village', name: 'Village', dimension: 'overworld', x: 800, y: null, z: -320, note: '' },
  { id: 'portal-a', name: 'Portal A', dimension: 'nether', x: -3, y: 70, z: 5, note: '' },
  { id: 'portal-b', name: 'Portal B', dimension: 'nether', x: 100, y: 70, z: -40, note: '' },
  { id: 'end', name: 'End', dimension: 'end', x: 0, y: null, z: 0, note: '' },
];

test('ranks all opposite-dimension candidates from floored negative Overworld projection', () => {
  const candidates = portalCandidates(places, 'home', 'overworld-to-nether');
  assert.deepEqual(candidates.map(item => item.location.id), ['portal-a', 'portal-b']);
  assert.deepEqual(candidates[0]?.projected, { x: -3, z: 5 });
  assert.equal(candidates[0]?.horizontalOffset, 0);
  assert.equal(candidates[0]?.yDelta, 0);
  assert.deepEqual(portalOriginLocations(places, 'overworld-to-nether').map(item => item.id), ['home', 'village']);
});

test('supports reverse projection, optional manual Y and no implied link for End locations', () => {
  const reverse = portalCandidates(places, 'portal-a', 'nether-to-overworld');
  assert.deepEqual(reverse[0]?.projected, { x: -24, z: 40 });
  assert.equal(reverse.find(item => item.location.id === 'village')?.yDelta, null);
  assert.deepEqual(portalCandidates(places, 'end', 'overworld-to-nether'), []);
});
