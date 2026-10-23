// SPDX-License-Identifier: MIT
import { mountWorkbench } from '../../../../packages/plugin-workbench/src/index.ts';

mountWorkbench({
  scope: 'options',
  title: { en: 'Controls Auditor', 'zh-CN': '操作检查器' },
  columns: [
    { key: 'name', label: { en: 'Option', 'zh-CN': '选项' } },
    { key: 'value', label: { en: 'Value', 'zh-CN': '值' } },
  ],
  metrics: [
    { key: '$count', label: { en: 'Options', 'zh-CN': '选项' } },
    { key: '$findings', label: { en: 'Control notes', 'zh-CN': '按键提示' } },
  ],
});
