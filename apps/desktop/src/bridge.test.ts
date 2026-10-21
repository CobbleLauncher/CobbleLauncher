// SPDX-License-Identifier: MIT
import { describe, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { connectDesktop } from './bridge';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));

describe('desktop connection boundary', () => {
  it('does not invoke a missing desktop host or fabricate a connection', async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await connectDesktop()).toEqual({ kind: 'preview' });
    expect(invoke).not.toHaveBeenCalled();
  });

  it('reports rejected IPC without echoing private host errors', async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockRejectedValue(new Error('private token and local path'));
    expect(await connectDesktop()).toEqual({ kind: 'unavailable' });
  });

  it('accepts the public bootstrap response only', async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    const info = { name: 'Cobble Launcher', version: '0.1.0', stage: 'development' };
    vi.mocked(invoke).mockResolvedValue(info);
    expect(await connectDesktop()).toEqual({ kind: 'connected', info });
    expect(invoke).toHaveBeenCalledWith('get_app_info');
  });

  it.each([null, {}, { name: 'Cobble', version: '0.1', stage: 'production' }, {
    name: 'Cobble', version: '0.1', stage: 'development', token: 'must reject',
  }])('rejects invalid or secret-bearing bootstrap data: %j', async (info) => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockResolvedValue(info);
    expect(await connectDesktop()).toEqual({ kind: 'unavailable' });
  });
});
