/** Production-build smoke tests; run with `-c playwright.preview.config.ts`. */
import { expect, test } from '@playwright/test';
import { loadExample, openApp, readCL } from './helpers';

test('production build boots the WASM worker and solves an airfoil', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  await openApp(page);
  await loadExample(page, 'NACA 0012');
  await page.getByRole('button', { name: /^NACA 0012/ }).click();
  const cl = await readCL(page);
  expect(cl).toBeGreaterThan(0.57);
  expect(cl).toBeLessThan(0.63);
  expect(errors).toEqual([]);
});

test('production build imports geometry through the worker', async ({ page }) => {
  await openApp(page);
  await page.getByRole('button', { name: 'Add' }).click();
  await page.getByRole('menuitem', { name: 'Import coordinates' }).click();
  await page.getByLabel('Coordinate text').fill('0,0\n1,1\n1,0\n0,1\n');
  await expect(page.getByText(/crosses another segment/)).toBeVisible();
});

test('dev-only store hook is not shipped', async ({ page }) => {
  await openApp(page);
  expect(await page.evaluate(() => 'aeroflow' in window || '__aeroflow' in window)).toBe(false);
});
