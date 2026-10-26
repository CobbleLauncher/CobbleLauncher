// SPDX-License-Identifier: MIT
import { describe, expect, it } from 'vitest';
import { findLinkedProject, linkedVersionId, parseFileInboxMarketEvent } from './file-inbox-market';

describe('File Inbox market handoff', () => {
  it('requires a bounded provider project and game for game content', () => {
    expect(parseFileInboxMarketEvent({ provider: 'modrinth', kind: 'mod', project: 'sodium', version: 'v1', gameId: 'g1' }))
      .toEqual({ provider: 'modrinth', kind: 'mod', project: 'sodium', version: 'v1', gameId: 'g1' });
    expect(parseFileInboxMarketEvent({ provider: 'modrinth', kind: 'mod', project: 'sodium' })).toBeNull();
    expect(parseFileInboxMarketEvent({ provider: 'curseforge', kind: 'shader_pack', project: 'x', gameId: 'g1' })).toBeNull();
    expect(parseFileInboxMarketEvent({ provider: 'curseforge', kind: 'modpack', project: 'x', version: 'latest' })).toBeNull();
  });
  it('only opens an exact search hit and keeps an unresolved version unselected', () => {
    const link = { provider: 'modrinth' as const, kind: 'mod' as const, project: 'sodium', version: 'v2', gameId: 'g1' };
    expect(findLinkedProject([{ slug: 'sodium-extra', project_id: 'a' }, { slug: 'sodium', project_id: 'b' }], link)?.project_id).toBe('b');
    expect(findLinkedProject([{ slug: 'sodium-extra', project_id: 'a' }], link)).toBeUndefined();
    expect(linkedVersionId([{ version_id: 'id1', version_number: 'v1' }, { version_id: 'id2', version_number: 'v2' }], link.version)).toBe('id2');
    expect(linkedVersionId([{ version_id: 'id1', version_number: 'v1' }], link.version)).toBe('');
  });
});
