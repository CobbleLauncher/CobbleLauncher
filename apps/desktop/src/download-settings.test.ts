// SPDX-License-Identifier: MIT
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { defaultDownloadSettings, readDownloadSettings, saveDownloadSettings, validateDownloadSettings } from './download-settings';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));

beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); });
describe('download source preferences', () => {
  it('keeps browser preview persistence separate from native IPC', async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await readDownloadSettings()).toEqual(defaultDownloadSettings());
    await saveDownloadSettings({ source: 'bmclapi', mirror: '' });
    expect(await readDownloadSettings()).toEqual({ source: 'bmclapi', mirror: '' });
    expect(invoke).not.toHaveBeenCalled();
  });
  it('uses typed native commands and propagates save failure', async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockResolvedValue({ source: 'bmclapi', mirror: '' });
    expect(await readDownloadSettings()).toEqual({ source: 'bmclapi', mirror: '' });
    expect(invoke).toHaveBeenCalledWith('get_download_settings');
    await saveDownloadSettings({ source: 'custom', mirror: ' https://mirror.example.invalid/ ' });
    expect(invoke).toHaveBeenCalledWith('set_download_settings', { settings: { source: 'custom', mirror: 'https://mirror.example.invalid/' } });
    vi.mocked(invoke).mockRejectedValue(new Error('disk full'));
    await expect(saveDownloadSettings(defaultDownloadSettings())).rejects.toThrow('disk full');
  });
  it('never substitutes preview defaults for invalid native data', async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockResolvedValue(null);
    await expect(readDownloadSettings()).rejects.toThrow('Invalid download settings');
    vi.mocked(invoke).mockRejectedValue(new Error('unreadable settings'));
    await expect(readDownloadSettings()).rejects.toThrow('unreadable settings');
  });
  it.each(['', 'http://mirror.example.invalid/', 'https://user:pass@mirror.example.invalid/', 'https://mirror.example.invalid/?token=x', 'https://mirror.example.invalid/#fragment'])('rejects invalid mirror %s', async (mirror) => {
    expect(validateDownloadSettings({ source: 'custom', mirror })).toBeTruthy();
    await expect(saveDownloadSettings({ source: 'custom', mirror })).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
  });
});
