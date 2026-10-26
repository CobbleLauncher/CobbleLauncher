// SPDX-License-Identifier: MIT
import { locale } from '../i18n';

const labels = {
  'zh-CN': {
    title: '分享改动', export: '导出', import: '导入', back: '返回', retry: '重试',
    source: '当前游戏', target: '目标游戏', chooseFile: '选择清单文件', chooseAgain: '重新选择文件',
    mods: 'Mod', configs: '配置文件', excluded: '不能分享的 Mod', selectAll: '全选', clear: '清空',
    selected: '已选', review: '预览清单', edit: '修改选择', save: '保存清单', saving: '正在保存…',
    load: '正在读取…', file: '清单', ready: '准备应用', preview: '预览改动', applying: '正在应用…', apply: '应用改动',
    added: '新增', replaced: '替换', skipped: '跳过', conflict: '配置选择',
    keep: '保留当前', adopt: '使用清单', current: '当前', incoming: '清单',
    localMods: '其他本地 Mod 保留', none: '没有可分享的项目', empty: '没有需要修改的内容',
    saved: '清单已保存', applied: '改动已应用', cancelled: '未选择文件',
    loadFailed: '无法读取当前游戏的内容', exportFailed: '保存失败', importFailed: '无法读取清单',
    previewFailed: '无法预览改动', applyFailed: '应用失败', stale: '内容已变化，请重新预览',
    locked: '游戏正在运行或处理内容，暂时不能更改', incompatible: '这个清单与目标游戏不兼容',
    excludedHelp: '这些 Mod 没有可分享的来源引用，未加入清单。',
    configHelp: '配置可能包含私人信息。请在保存前检查选中的文件。',
    noGames: '没有可用的目标游戏', allGamesFailed: '无法读取游戏列表',
    version: '版本', from: '原版本', to: '新版本',
    partial: '部分改动已应用', undoMods: '撤销 Mod 改动', restoreConfigs: '恢复配置', undoNewConfig: '移除新增配置', recoveryFailed: '恢复失败，请检查游戏内容',
    partialConfig: '有配置未完成。已应用的内容如下。', finish: '完成',
    leavePartial: '部分改动已应用', leavePartialHelp: '离开后仍可从维护记录撤销 Mod；新增配置的本次恢复入口会失效。', leave: '离开',
  },
  'en-US': {
    title: 'Share changes', export: 'Export', import: 'Import', back: 'Back', retry: 'Retry',
    source: 'Current game', target: 'Target game', chooseFile: 'Choose list file', chooseAgain: 'Choose another file',
    mods: 'Mods', configs: 'Config files', excluded: 'Mods that cannot be shared', selectAll: 'Select all', clear: 'Clear',
    selected: 'Selected', review: 'Review list', edit: 'Edit selection', save: 'Save list', saving: 'Saving…',
    load: 'Loading…', file: 'List', ready: 'Ready to apply', preview: 'Preview changes', applying: 'Applying…', apply: 'Apply changes',
    added: 'Add', replaced: 'Replace', skipped: 'Skip', conflict: 'Config choices',
    keep: 'Keep current', adopt: 'Use list', current: 'Current', incoming: 'List',
    localMods: 'Other local Mods stay', none: 'Nothing available to share', empty: 'No changes needed',
    saved: 'List saved', applied: 'Changes applied', cancelled: 'No file selected',
    loadFailed: 'Could not read this game', exportFailed: 'Could not save the list', importFailed: 'Could not read the list',
    previewFailed: 'Could not preview changes', applyFailed: 'Could not apply changes', stale: 'Content changed. Preview again.',
    locked: 'This game is running or changing content', incompatible: 'This list does not fit the target game',
    excludedHelp: 'These Mods have no shareable source reference and are excluded.',
    configHelp: 'Config files may contain private information. Check each selected file before saving.',
    noGames: 'No target games available', allGamesFailed: 'Could not load games',
    version: 'Version', from: 'Current version', to: 'New version',
    partial: 'Some changes were applied', undoMods: 'Undo Mod changes', restoreConfigs: 'Restore configs', undoNewConfig: 'Remove new configs', recoveryFailed: 'Recovery failed. Check game content.',
    partialConfig: 'Some configs did not finish. Applied content is shown below.', finish: 'Done',
    leavePartial: 'Some changes were applied', leavePartialHelp: 'Mod changes remain in history. Recovery for newly added configs is only available on this screen.', leave: 'Leave',
  },
} as const;
export type LabelKey = keyof typeof labels['en-US'];
export function label(key: LabelKey): string { return labels[locale.value === 'en-US' ? 'en-US' : 'zh-CN'][key]; }
