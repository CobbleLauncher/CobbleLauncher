// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import { marketImage } from './market-images';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.resetAllMocks(); vi.restoreAllMocks(); });
const old = 'data:image/png;base64,old';
const fresh = 'data:image/png;base64,fresh';

it('shows expired images immediately and merges background refreshes', async () => {
  let now = 0;
  vi.spyOn(Date, 'now').mockImplementation(() => now);
  vi.mocked(invoke).mockResolvedValueOnce(old);
  expect(await marketImage('modrinth', 'https://example.test/stale', 'preview')).toBe(old);
  now = 300_001;
  let finish!: (value: string) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(resolve => { finish = resolve as (value: string) => void; }));
  expect(await marketImage('modrinth', 'https://example.test/stale', 'preview')).toBe(old);
  expect(await marketImage('modrinth', 'https://example.test/stale', 'preview')).toBe(old);
  expect(invoke).toHaveBeenCalledTimes(2);
  finish(fresh); await flushPromises();
  expect(await marketImage('modrinth', 'https://example.test/stale', 'preview')).toBe(fresh);
  expect(invoke).toHaveBeenCalledTimes(2);
});

it('retains an expired image when refreshing offline', async () => {
  let now = 0;
  vi.spyOn(Date, 'now').mockImplementation(() => now);
  vi.mocked(invoke).mockResolvedValueOnce(old);
  await marketImage('modrinth', 'https://example.test/offline', 'icon');
  now = 300_001; vi.mocked(invoke).mockRejectedValue(new Error('offline'));
  expect(await marketImage('modrinth', 'https://example.test/offline', 'icon')).toBe(old);
  await flushPromises();
  expect(await marketImage('modrinth', 'https://example.test/offline', 'icon')).toBe(old);
  await flushPromises();
});

it('keeps provider and image variant caches separate', async () => {
  vi.mocked(invoke).mockResolvedValue(old);
  await Promise.all([marketImage('modrinth', 'https://example.test/shared', 'icon'), marketImage('curseforge', 'https://example.test/shared', 'icon'), marketImage('modrinth', 'https://example.test/shared', 'preview')]);
  expect(invoke).toHaveBeenCalledTimes(3);
});
