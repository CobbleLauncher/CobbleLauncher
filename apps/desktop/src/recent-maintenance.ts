// SPDX-License-Identifier: MIT
import { snapshotTimestamp } from './world-snapshots';
export type MaintenanceKind = 'mod' | 'config' | 'world' | 'clean_exit';
export type MaintenanceItem = { kind: MaintenanceKind; id: string; createdAt: number | string; title: string; status: string };
export type MaintenanceOverview = { items: MaintenanceItem[]; unavailable: string[]; limited: boolean };
export function recentMaintenance(items: MaintenanceItem[], limit = 40): (MaintenanceItem & { at: number })[] {
  return items.map(item => ({ ...item, at: typeof item.createdAt === 'number' ? item.createdAt : snapshotTimestamp(item.createdAt) }))
    .filter(item => Number.isFinite(item.at) && item.at > 0)
    .sort((a, b) => b.at - a.at || a.id.localeCompare(b.id)).slice(0, limit);
}
export const maintenanceCopy = {
  'en-US': { title: 'Recent maintenance', empty: 'No retained records.', failed: 'Could not load maintenance records.', partial: 'Some records could not be loaded.', limited: 'Showing recent retained records.', retry: 'Retry', mod: 'Mod change', config: 'Config backup', world: 'World snapshot', clean_exit: 'Clean exit', undone: 'Undone', applied: 'Applied', undoing: 'Undo incomplete', installing: 'Install incomplete', automatic: 'Automatic' },
  'zh-CN': { title: '近期维护', empty: '暂无保留记录。', failed: '无法读取维护记录。', partial: '部分记录未能读取。', limited: '显示近期保留的记录。', retry: '重试', mod: 'Mod 变更', config: '配置备份', world: '世界快照', clean_exit: '正常退出', undone: '已撤销', applied: '已应用', undoing: '撤销未完成', installing: '安装未完成', automatic: '自动' },
} as const;
