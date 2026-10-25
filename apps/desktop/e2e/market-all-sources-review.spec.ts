// SPDX-License-Identifier: MIT
import { expect, test } from '@playwright/test';

test('market renders both sources and visible target at narrow and wide widths', async ({ page }, info) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  for (const [lang, theme, width] of [['en-US', 'light', 1100], ['zh-CN', 'dark', 560]] as const) {
    await page.setViewportSize({ width, height: 800 });
    await page.goto(`/e2e/fixtures/market-pins.html?lang=${lang}&theme=${theme}`);
    await expect(page.locator('.market-source-title')).toHaveText(['Modrinth', 'CurseForge']);
    await expect(page.locator('.market-source-results').first()).toContainText('Forest Map');
    await expect(page.locator('.market-source-results').last()).toContainText('Forest Map CF');
    await expect(page.locator('.market-shared-search input')).toBeVisible();
    await expect(page.locator('.market-source-results input[type="search"]')).toHaveCount(0);
    if (width === 560) await expect(page.locator('.market-mobile-target select')).toBeVisible();
    else await expect(page.locator('#market-game-desktop')).toBeVisible();
    await page.screenshot({ path: info.outputPath(`market-all-${lang}-${theme}-${width}.png`), fullPage: true, animations: 'disabled' });
    await page.locator('.market-shared-search input').fill('forest');
    await page.locator('.market-shared-search button').click();
    await expect.poll(() => page.evaluate(() => (window as unknown as { __searchCalls: { command: string; args: { query: string } }[] }).__searchCalls
      .filter(call => call.args.query === 'forest').map(call => call.command).sort())).toEqual(['search_curseforge', 'search_game_mods']);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.goto(`/e2e/fixtures/market-pins.html?kind=modpack&lang=${lang}&theme=${theme}`);
    await expect(page.locator('.market-source-title')).toHaveText(['Modrinth', 'CurseForge']);
    await expect(page.locator('.pack-discovery-filter-details')).toHaveAttribute('open', '');
    await expect(page.locator('.cf-filter-details')).toHaveAttribute('open', '');
    await page.screenshot({ path: info.outputPath(`market-modpacks-${lang}-${theme}-${width}.png`), fullPage: true, animations: 'disabled' });
    await page.locator('.market-shared-search input').fill('quest');
    await page.locator('.market-shared-search button').click();
    await expect.poll(() => page.evaluate(() => (window as unknown as { __searchCalls: { command: string; args: { query: string } }[] }).__searchCalls
      .filter(call => call.args.query === 'quest').map(call => call.command).sort())).toEqual(['search_curseforge', 'search_modrinth_packs']);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  expect(errors).toEqual([]);
});
