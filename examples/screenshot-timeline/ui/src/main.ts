// SPDX-License-Identifier: MIT
import { mountWorkbench } from '../../../../packages/plugin-workbench/src/index.ts';

mountWorkbench({
  scope: 'screenshots',
  title: { en: 'Screenshot Timeline', 'zh-CN': '截图时间线' },
  columns: [
    { key: 'name', label: { en: 'Screenshot', 'zh-CN': '截图' } },
    { key: 'modifiedAt', label: { en: 'Date', 'zh-CN': '日期' }, format: 'date' },
    { key: 'sizeBytes', label: { en: 'Size', 'zh-CN': '大小' }, format: 'bytes' },
    { key: 'width', label: { en: 'Width', 'zh-CN': '宽度' } },
    { key: 'height', label: { en: 'Height', 'zh-CN': '高度' } },
  ],
  metrics: [
    { key: '$count', label: { en: 'Screenshots', 'zh-CN': '截图' } },
    { key: '$bytes', label: { en: 'Total size', 'zh-CN': '总大小' }, format: 'bytes' },
    { key: '$findings', label: { en: 'Timeline notes', 'zh-CN': '时间线提示' } },
  ],
});
