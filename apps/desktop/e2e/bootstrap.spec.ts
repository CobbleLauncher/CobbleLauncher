// SPDX-License-Identifier: MIT
import { test, expect } from '@playwright/test';

test('first render uses no external network or simulated game state', async ({ page }) => {
  const remoteRequests: string[] = [];
  page.on('request', (request) => {
    if (new URL(request.url()).hostname !== '127.0.0.1') remoteRequests.push(request.url());
  });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: '还没有游戏' })).toBeVisible();
  await expect(page.locator('body')).not.toContainText('本地界面已就绪');
  await expect(page.getByRole('button', { name: '创建游戏' })).toBeDisabled();
  await expect(page.getByRole('button', { name: '导入整合包' })).toBeDisabled();
  await page.context().setOffline(true);
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('radio', { name: '深色', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(page.getByRole('button', { name: '打开设置' })).toBeFocused();
  expect(remoteRequests).toEqual([]);
});

test('appearance survives reload and narrow layouts do not overflow', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('radio', { name: '深色', exact: true }).click();
  await page.getByRole('button', { name: '完成' }).click();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  for (const width of [560, 340]) {
    await page.setViewportSize({ width, height: 700 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  }
});

test('light and dark shell previews', async ({ page }, testInfo) => {
  await page.goto('/');
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('radio', { name: '浅色', exact: true }).click();
  await page.getByRole('button', { name: '完成' }).click();
  await page.screenshot({ path: testInfo.outputPath('shell-light.png') });
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('radio', { name: '深色', exact: true }).click();
  await page.getByRole('button', { name: '完成' }).click();
  await page.screenshot({ path: testInfo.outputPath('shell-dark.png') });
});

test('settings traps keyboard focus and respects reduced motion', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  expect(await page.locator('.empty-library').evaluate((element) => getComputedStyle(element).animationName)).toBe('none');
  await page.getByRole('button', { name: '打开设置' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  expect(await dialog.evaluate((element) => getComputedStyle(element).animationName)).toBe('none');
  for (const key of ['Tab', 'Tab', 'Tab', 'Tab', 'Tab', 'Shift+Tab', 'Shift+Tab']) {
    await page.keyboard.press(key);
    expect(await dialog.evaluate((element) => element.contains(document.activeElement))).toBe(true);
  }
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: '打开设置' })).toBeFocused();
});

test('minimum window and 200 percent CSS zoom preserve settings access', async ({ page }) => {
  await page.goto('/');
  await page.setViewportSize({ width: 560, height: 420 });
  await expect(page.getByRole('button', { name: '打开设置' })).toBeInViewport();
  await expect(page.getByRole('button', { name: '创建游戏' })).toBeInViewport();
  await page.setViewportSize({ width: 680, height: 840 });
  // CSS zoom exercises layout scaling; native/browser UI zoom is not simulated.
  await page.locator('html').evaluate((element) => { element.style.zoom = '2'; });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.getByRole('button', { name: '打开设置' }).click();
  await expect(page.getByRole('button', { name: '完成' })).toBeInViewport();
  await page.getByRole('radio', { name: '深色', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: '完成' }).click();
});
