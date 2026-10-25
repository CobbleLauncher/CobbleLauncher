// SPDX-License-Identifier: MIT
import { expect, test } from '@playwright/test';

test('narrow English Mod detail uses a localized back control', async ({ page }, info) => {
  await page.setViewportSize({ width: 560, height: 800 });
  await page.goto('/e2e/fixtures/market-pins.html?lang=en-US&theme=light');
  await page.locator('.market-saved-mobile').first().getByRole('button', { name: 'Forest Map', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Back to results' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Back to results' }).locator('svg')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Save for later' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Remove favorite' })).toBeVisible();
  await page.screenshot({ path: info.outputPath('mod-detail-en-light-560.png'), fullPage: true, animations: 'disabled' });
});
