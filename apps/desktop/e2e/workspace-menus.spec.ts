// SPDX-License-Identifier: MIT
import { test, expect } from '@playwright/test';
test('desktop context, edit and long-press menus respect targets and cancellation', async ({ page }) => {
  await page.goto('/e2e/fixtures/workspace-menus.html');
  await page.locator('#blank').click({ button: 'right', position: { x: 120, y: 150 } });
  await expect(page.getByRole('menuitem', { name: 'Add game', exact: true })).toBeVisible();
  await expect(page.getByText('Print', { exact: true })).toHaveCount(0);
  await page.keyboard.press('Escape');
  await page.locator('#own').click({ button: 'right' }); await expect(page.getByRole('menu')).toHaveCount(0);
  await page.locator('#edit').click({ button: 'right' });
  await expect(page.getByRole('menuitem', { name: 'Paste', exact: true })).toBeVisible();
  await expect(page.getByRole('menuitem', { name: 'Add game', exact: true })).toHaveCount(0);
  await page.keyboard.press('Escape'); await expect(page.locator('#edit')).toBeFocused();
  const box = (await page.locator('#blank').boundingBox())!;
  await page.mouse.move(box.x+180,box.y+100); await page.mouse.down(); await page.mouse.move(box.x+210,box.y+100); await page.waitForTimeout(600); await page.mouse.up(); await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.mouse.move(box.x+180,box.y+100); await page.mouse.down(); await page.waitForTimeout(600); await page.mouse.up();
  await expect(page.getByRole('dialog')).toBeVisible(); await page.screenshot({path:'../../validation/t078-desktop-menus/radial-1100-light-en.png'});
  await page.keyboard.press('Escape'); await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.locator('#blank').click({ button: 'right', position:{x:100,y:100} }); await page.getByRole('menuitem',{name:'Add game',exact:true}).click();
  expect(await page.evaluate(()=>(window as unknown as {menuRuns:string[]}).menuRuns)).toEqual(['create']);
});
test('menus stay inside a zoomed narrow viewport and keyboard activates one radial action', async({page})=>{
  await page.setViewportSize({width:560,height:800}); await page.goto('/e2e/fixtures/workspace-menus.html?scale=125&theme=dark&locale=zh-CN');
  await page.locator('#blank').click({button:'right',position:{x:350,y:180}});
  const menu=page.getByRole('menu'); await expect(menu).toBeVisible(); const bounds=(await menu.boundingBox())!; expect(bounds.x).toBeGreaterThanOrEqual(0); expect(bounds.x+bounds.width).toBeLessThanOrEqual(560);
  await menu.evaluate(async element => { await Promise.all(element.getAnimations().map(animation => animation.finished)); });
  await page.screenshot({path:'../../validation/t078-desktop-menus/context-560-dark-zh.png'}); await page.keyboard.press('Escape');
  await page.mouse.move(420,550); await page.mouse.down(); await page.waitForTimeout(600); await page.mouse.up(); await expect(page.getByRole('dialog')).toBeVisible();
  const radial=(await page.getByRole('dialog').boundingBox())!; expect(radial.x).toBeGreaterThanOrEqual(0); expect(radial.x+radial.width).toBeLessThanOrEqual(560); expect(radial.y+radial.height).toBeLessThanOrEqual(800);
  await page.screenshot({path:'../../validation/t078-desktop-menus/radial-560-dark-zh.png'});
  await page.keyboard.press('ArrowRight'); await page.keyboard.press('Enter'); expect(await page.evaluate(()=>(window as unknown as {menuRuns:string[]}).menuRuns)).toHaveLength(1);
});
