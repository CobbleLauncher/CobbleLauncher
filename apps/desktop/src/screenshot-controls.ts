// SPDX-License-Identifier: MIT
import { locale } from './i18n';

const labels = {
  'en-US': { zoom: 'Zoom', zoomIn: 'Zoom in', zoomOut: 'Zoom out', resetZoom: 'Reset zoom', fit: 'Fit', copy: 'Copy image', copied: 'Image copied', copyFailed: 'Could not copy image. Try again.' },
  'zh-CN': { zoom: '缩放', zoomIn: '放大', zoomOut: '缩小', resetZoom: '重置缩放', fit: '适应窗口', copy: '复制图片', copied: '图片已复制', copyFailed: '无法复制图片，请重试。' },
} as const;

export function screenshotControlLabel(key: keyof typeof labels['en-US']) {
  return labels[locale.value][key];
}
