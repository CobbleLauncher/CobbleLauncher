// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { applyHostAppearance } from './appearance.ts';
import type { Context } from './index.ts';

test('bundled frontend opts into validated host appearance without changing a missing preference', () => {
  const properties = new Map<string, string>();
  const root = { dataset: {} as Record<string, string>, style: { setProperty: (key: string, value: string) => properties.set(key, value), removeProperty: (key: string) => properties.delete(key) } } as unknown as HTMLElement;
  const context = { theme: 'dark', appearance: { accent: 'blue', density: 'compact', scale: 125, motion: 'reduced' } } as Context;
  assert.equal(applyHostAppearance(context, root), context);
  assert.deepEqual(root.dataset, { hostAppearance: 'true', theme: 'dark', accent: 'blue', density: 'compact', motion: 'reduced', hostScale: '125', hostTextSize: '14' });
  assert.equal(properties.get('--font-size-base'), '14px');
  assert.equal(properties.get('--appearance-scale'), '1.25');
  applyHostAppearance({ theme: 'light' } as Context, root);
  assert.deepEqual(root.dataset, { hostAppearance: 'true', theme: 'light' });
  assert.equal(properties.size, 0);
});

test('applies new text size, comfortable density and intermediate scale only on the opted-in root', () => {
  const properties = new Map<string, string>();
  const root = { dataset: {} as Record<string, string>, style: { setProperty: (key: string, value: string) => properties.set(key, value), removeProperty: (key: string) => properties.delete(key) } } as unknown as HTMLElement;
  const context = { theme: 'light', appearance: { accent: 'purple', density: 'comfortable', scale: 105, textSize: 18, motion: 'system' } } as Context;
  applyHostAppearance(context, root);
  assert.equal(root.dataset.density, 'comfortable');
  assert.equal(root.dataset.hostScale, '105');
  assert.equal(properties.get('--font-size-base'), '18px');
  assert.equal(properties.get('--appearance-scale'), '1.05');
});
