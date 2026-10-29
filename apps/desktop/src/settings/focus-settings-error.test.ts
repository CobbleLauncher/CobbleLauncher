// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { focusSettingsError } from './focus-settings-error';
it('focuses the active category error rather than an earlier hidden draft', () => {
  const root = document.createElement('div');
  root.innerHTML = '<div role="tabpanel" data-state="inactive" hidden><input aria-invalid="true" id="old-error"></div><div role="tabpanel" data-state="active"><input aria-invalid="true" id="current-error"></div>';
  document.body.append(root);
  focusSettingsError(root); expect(document.activeElement?.id).toBe('current-error'); root.remove();
});
