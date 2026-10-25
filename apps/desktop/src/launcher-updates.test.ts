// SPDX-License-Identifier: MIT
import { beforeEach, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { checkLauncherUpdateAfterHome, launcherUpdateErrorKey } from './launcher-updates';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => vi.resetAllMocks());
it('maps host failures without exposing unknown paths or network errors', () => {
  expect(launcherUpdateErrorKey('UPDATE_BUSY')).toBe('launcherUpdates.errors.UPDATE_BUSY');
  expect(launcherUpdateErrorKey('C:/private/file')).toBe('launcherUpdates.errors.UPDATE_UNKNOWN');
});
it('does not call the host from a browser preview', async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  expect(await checkLauncherUpdateAfterHome()).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});
it('does not check without opt-in and a configured feed', async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  for (const status of [{ background_check: false, configured: true }, { background_check: true, configured: false }]) {
    vi.mocked(invoke).mockResolvedValue(status);
    expect(await checkLauncherUpdateAfterHome()).toBeNull();
  }
  expect(vi.mocked(invoke).mock.calls.every(([command]) => command === 'launcher_update_status')).toBe(true);
});
it('checks after opt-in and preserves host failure', async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockResolvedValueOnce({ background_check: true, configured: true }).mockRejectedValueOnce(new Error('offline'));
  await expect(checkLauncherUpdateAfterHome()).rejects.toThrow('offline');
  expect(invoke).toHaveBeenLastCalledWith('check_launcher_update', { automatic: true });
});
