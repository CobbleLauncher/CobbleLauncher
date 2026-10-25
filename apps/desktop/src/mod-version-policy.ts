// SPDX-License-Identifier: MIT
export type ModUpdateChannel = 'stable' | 'preview';
export type ProjectVersionPolicy = { channel?: ModUpdateChannel; ignored_versions: string[] };
export type ModUpdatePolicy = { schema_version: 1; channel: ModUpdateChannel; projects: Record<string, ProjectVersionPolicy> };
export function emptyModUpdatePolicy(): ModUpdatePolicy { return { schema_version: 1, channel: 'stable', projects: {} }; }
export function projectPolicyKey(projectId: string, source = 'modrinth'): string { return `${source}:${projectId}`; }
export function cloneModUpdatePolicy(policy: ModUpdatePolicy): ModUpdatePolicy {
  return { schema_version: 1, channel: policy.channel, projects: Object.fromEntries(Object.entries(policy.projects).map(([key, value]) => [key, { ...value, ignored_versions: [...value.ignored_versions] }])) };
}
export function changeIgnoredVersion(policy: ModUpdatePolicy, key: string, versionId: string, ignored: boolean): ModUpdatePolicy {
  const next = cloneModUpdatePolicy(policy);
  const entry = next.projects[key] ?? { ignored_versions: [] };
  entry.ignored_versions = ignored ? [...new Set([...entry.ignored_versions, versionId])] : entry.ignored_versions.filter(id => id !== versionId);
  if (entry.ignored_versions.length || entry.channel) next.projects[key] = entry;
  else delete next.projects[key];
  return next;
}
export const versionPolicyCopy = {
  'en-US': {
    title: 'Update preferences', channel: 'Update channel', projectChannel: 'This Mod', inherit: 'Use game preference', stable: 'Stable releases', preview: 'Include previews',
    ignore: 'Ignore this update', resume: 'Allow this version', ignored: 'Ignored versions', dependencies: 'Required dependencies can still use ignored versions.', failed: 'Could not save update preferences.', loadFailed: 'Could not load update preferences.', retry: 'Retry',
  },
  'zh-CN': {
    title: '更新偏好', channel: '更新通道', projectChannel: '此 Mod', inherit: '跟随游戏偏好', stable: '正式版', preview: '包含预览版',
    ignore: '忽略此更新', resume: '允许此版本', ignored: '已忽略版本', dependencies: '必需依赖仍可使用已忽略的版本。', failed: '无法保存更新偏好。', loadFailed: '无法读取更新偏好。', retry: '重试',
  },
} as const;
