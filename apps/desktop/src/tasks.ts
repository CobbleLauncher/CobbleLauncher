// SPDX-License-Identifier: MIT
export type TaskProgress = { phase: string; completed: number; total: number; reused: number };
export type TaskSnapshot = {
  id: string;
  game_id: string | null;
  game_name: string;
  title: string;
  kind: 'game_install' | 'mod_install' | 'mod_update' | 'resource_pack' | 'shader_pack' | 'modpack_download';
  state: 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted';
  progress: TaskProgress | null;
  error: string | null;
  can_retry: boolean;
  created_at: number;
  updated_at: number;
  has_result: boolean;
};
export function isActiveTask(task: TaskSnapshot): boolean { return task.state === 'running' || task.state === 'cancelling'; }
export function taskStateLabel(task: TaskSnapshot): string {
  return { running: '进行中', cancelling: '正在取消', succeeded: '已完成', failed: '失败', cancelled: '已取消', interrupted: '已中断' }[task.state];
}
export function taskPhaseLabel(progress: TaskProgress): string {
  if (progress.phase === 'pack-base-complete') return '准备整合包文件';
  const labels: Record<string, string> = { metadata: '读取信息', files: '下载文件', assets: '下载游戏资源', fabric: '安装 Fabric', 'forge-installer': '安装 Forge', 'neoforge-installer': '安装 NeoForge', 'pack-download': '下载内容文件', 'pack-overrides': '复制整合包配置', 'pack-install': '安装内容', 'mod-download': '下载 Mod', 'mod-install': '安装 Mod', 'import-copy': '复制游戏内容', complete: '安装完成' };
  return labels[progress.phase.replace(/^pack-base-/, '')] ?? '正在处理';
}
export function taskHasKnownTotal(progress: TaskProgress): boolean {
  const phase = progress.phase.replace(/^pack-base-/, '');
  return progress.total > 0 && phase !== 'forge-installer' && phase !== 'neoforge-installer';
}
