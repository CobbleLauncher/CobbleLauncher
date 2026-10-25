// SPDX-License-Identifier: MIT
export const gameDirectoryCopy = {
  'zh-CN': {
    title: '游戏安装位置', recommended: '推荐', browse: '选择文件夹', standard: '使用推荐位置',
    available: '可用', unknown: '暂时无法读取剩余空间', loading: '正在读取安装位置…',
    low: '剩余空间较少，大型整合包可能需要更多空间。',
    loadFailed: '无法读取安装位置，请重试。', pickFailed: '无法打开文件夹选择，请重试。',
    saveFailed: '无法保存安装位置，请检查文件夹权限后重试。', retry: '重试',
  },
  'en-US': {
    title: 'Game installation folder', recommended: 'Recommended', browse: 'Choose folder', standard: 'Use recommended folder',
    available: 'Free', unknown: 'Free space is currently unavailable', loading: 'Reading installation folder…',
    low: 'Free space is low. Large modpacks may need more room.',
    loadFailed: 'Could not read the installation folder. Try again.', pickFailed: 'Could not open the folder picker. Try again.',
    saveFailed: 'Could not save this folder. Check its permissions and try again.', retry: 'Retry',
  },
} as const;
