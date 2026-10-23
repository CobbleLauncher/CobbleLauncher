// SPDX-License-Identifier: MIT
import { webcrypto } from 'node:crypto';
import { expect, it, vi } from 'vitest';
import { meshIdentityCard, meshIdentityDigest, meshIdentityTiles } from './mesh-social';

it('hashes the entire identity with SHA-256, including IDs sharing the visible suffix', async () => {
  vi.stubGlobal('crypto', webcrypto);
  expect(await meshIdentityDigest('abc')).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
  const first = await meshIdentityDigest('first-full-peer-id-shared-suffix');
  const second = await meshIdentityDigest('second-full-peer-id-shared-suffix');
  expect(first).not.toBe(second);
  expect(meshIdentityTiles(first)).not.toEqual(meshIdentityTiles(second));
  vi.unstubAllGlobals();
});
it('copies a versioned public identity card containing the full ID', () => {
  const profile = { peer_id: 'full-public-peer-id', display_name: 'Shared name', suffix: 'short-id', revision: 1 };
  expect(JSON.parse(meshIdentityCard(profile))).toEqual({ type: 'cobble-mesh-identity', version: 1, peerId: profile.peer_id, displayName: profile.display_name });
  expect(meshIdentityCard(profile)).not.toContain('short-id');
});
