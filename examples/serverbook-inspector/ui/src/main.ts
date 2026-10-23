// SPDX-License-Identifier: MIT
import { mountWorkbench } from '../../../../packages/plugin-workbench/src/index';

mountWorkbench({
  scope: 'servers',
  title: { en: 'Serverbook Inspector', 'zh-CN': '服务器列表检查' },
});
