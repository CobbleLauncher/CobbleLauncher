// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { recentMaintenance, type MaintenanceItem } from './recent-maintenance';
it('merges existing receipt timestamps with annotated snapshot dates and bounds recent results', () => {
  const entries: MaintenanceItem[] = [
    { kind: 'config', id: 'config', createdAt: Date.parse('2026-10-25T00:00:00Z'), title: 'settings.json', status: 'retained' },
    { kind: 'world', id: 'snapshot', createdAt: '2026-10-25T09:00:00+08:00[Asia/Shanghai]', title: 'World', status: 'automatic' },
    { kind: 'clean_exit', id: 'exit', createdAt: Date.parse('2026-10-25T02:00:00Z'), title: '', status: 'exit_zero' },
    { kind: 'mod', id: 'invalid', createdAt: 'not-a-date', title: 'Unknown', status: 'applied' },
  ];
  expect(recentMaintenance(entries, 2).map(item => item.id)).toEqual(['exit', 'snapshot']);
  expect(entries).toHaveLength(4);
  expect(recentMaintenance(Array.from({ length: 60 }, (_, i) => ({ ...entries[0]!, id: String(i), createdAt: i + 1 })))).toHaveLength(40);
});
