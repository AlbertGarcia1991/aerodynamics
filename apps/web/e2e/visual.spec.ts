/**
 * Visual regression baselines (PRD §64). The first run writes the baselines
 * under `e2e/visual.spec.ts-snapshots/`; later runs compare against them.
 * Animated particles are disabled so frames are deterministic.
 */
import { expect, test, type Page } from '@playwright/test';
import { expectTheme, loadExample, openApp } from './helpers';

async function stillFrame(page: Page): Promise<void> {
  // Particles off (deterministic), full-resolution pass finished.
  await page.getByRole('toolbar', { name: 'Visualisation' }).getByRole('button', { name: 'Particles' }).click();
  await expect(page.getByText('Preview resolution')).toBeHidden({ timeout: 10_000 });
  await page.waitForTimeout(400);
}

// 0.4 %: the old 2 % budget let a whole missing streamline layer (~1.5 % of pixels) pass.
const shot = { maxDiffPixelRatio: 0.004, animations: 'disabled' as const };

test.describe('visual baselines', () => {
  test('empty state', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'Uniform flow');
    // Particles animate continuously; freeze them like every other baseline.
    await stillFrame(page);
    await expect(page).toHaveScreenshot('empty-state.png', shot);
  });

  test('elementary flow: source and sink', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'Source and sink');
    await stillFrame(page);
    await expect(page).toHaveScreenshot('elementary-flow.png', shot);
  });

  test('airfoil velocity field', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await stillFrame(page);
    await expect(page).toHaveScreenshot('airfoil-velocity.png', shot);
  });

  test('airfoil pressure field', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.keyboard.press('3');
    await stillFrame(page);
    await expect(page).toHaveScreenshot('airfoil-pressure.png', shot);
  });

  test('airfoil Cp field with the body selected', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.keyboard.press('4');
    await page.getByRole('button', { name: /^NACA 0012/ }).click();
    await stillFrame(page);
    await expect(page).toHaveScreenshot('airfoil-cp-selected.png', shot);
  });

  test('dark theme', async ({ page }) => {
    await openApp(page, { theme: 'dark' });
    await expectTheme(page, 'dark');
    await stillFrame(page);
    await expect(page).toHaveScreenshot('dark-theme.png', shot);
  });

  test('error state: invalid geometry', async ({ page }) => {
    await openApp(page);
    await page.getByRole('button', { name: 'Add' }).click();
    await page.getByRole('menuitem', { name: 'Import coordinates' }).click();
    await page.getByLabel('Coordinate text').fill('0,0\n1,1\n1,0\n0,1\n');
    await expect(page.getByText(/crosses another segment/)).toBeVisible();
    await expect(page).toHaveScreenshot('error-state.png', shot);
  });
});
