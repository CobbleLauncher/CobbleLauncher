// SPDX-License-Identifier: MIT
import { expect, test } from '@playwright/test';

test('source renders saved market items and Mod pins across locales, themes and widths', async ({ page }, info) => {
  test.setTimeout(120_000);
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  for (const lang of ['en-US', 'zh-CN']) for (const theme of ['light', 'dark']) for (const width of [560, 1100]) {
    await page.setViewportSize({ width, height: 800 });
    await page.goto(`/e2e/fixtures/market-pins.html?lang=${lang}&theme=${theme}`);
    const collections = width === 560 ? page.locator('.market-saved-mobile') : page.locator('.market-sidebar .market-saved');
    await expect(collections.first()).toBeVisible();
    await expect(collections.first()).toContainText('Forest Map CF');
    await expect(collections.first()).toContainText('Forest Map');
    await expect(collections.first()).not.toContainText('Leaf Textures');
    await expect(collections.nth(1)).toContainText('Leaf Textures');
    await page.screenshot({ path: info.outputPath(`market-${lang}-${theme}-${width}.png`), fullPage: true, animations: 'disabled' });
    const saved = collections.first();
    await saved.getByRole('button', { name: 'Forest Map', exact: true }).click();
    await expect(page.locator('.discover-content').getByRole('heading', { name: 'Forest Map' })).toBeVisible();
    await page.screenshot({ path: info.outputPath(`market-detail-${lang}-${theme}-${width}.png`), fullPage: true, animations: 'disabled' });
    await page.goto(`/e2e/fixtures/market-pins.html?view=pins&lang=${lang}&theme=${theme}`);
    await page.locator('.mod-recognition-tools button').first().click();
    await expect(page.locator('.managed-mod-list')).toContainText(lang === 'en-US' ? 'Pinned' : '已固定');
    await page.screenshot({ path: info.outputPath(`mod-pins-${lang}-${theme}-${width}.png`), fullPage: true, animations: 'disabled' });
    await page.locator('.managed-mod-list li').first().getByRole('button').first().click();
    await expect(page.locator('.managed-mod-list li').first()).toContainText(lang === 'en-US' ? 'Pin version' : '固定版本');
    expect(await page.evaluate(() => (window as unknown as { __pinCalls: { projectId: string; pinned: boolean }[] }).__pinCalls)).toEqual([{ id: 'review-game', projectId: 'shared-project', pinned: false }]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  expect(errors).toEqual([]);
});
