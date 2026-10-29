// SPDX-License-Identifier: MIT
/** Focus the actual failed control after its category has been made visible. */
export function focusSettingsError(root: HTMLElement | undefined) {
  const scope = root?.querySelector<HTMLElement>('[role="tabpanel"][data-state="active"]') ?? root;
  const control = scope?.querySelector<HTMLElement>(
    'input[aria-invalid="true"]:not(:disabled), textarea[aria-invalid="true"]:not(:disabled), .settings-row-invalid input:not(:disabled), .settings-row-invalid select:not(:disabled), .settings-row-invalid textarea:not(:disabled), .settings-row-invalid button:not(:disabled)',
  );
  const target = control ?? scope?.querySelector<HTMLElement>('.settings-error, [role="alert"]') ?? root?.querySelector<HTMLElement>('.settings-save-bar [role="alert"]');
  if (!target) return;
  if (!control) target.tabIndex = -1;
  target.focus({ preventScroll: true });
  target.scrollIntoView?.({ block: 'nearest' });
}
