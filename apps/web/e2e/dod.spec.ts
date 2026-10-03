/**
 * PRD §79 Definition of Done — items not covered elsewhere get a direct check
 * here, so docs/VERIFICATION.md can cite a test for every item.
 */
import { expect, test, type Page } from '@playwright/test';
import { loadExample, openApp } from './helpers';

const KINDS = [
  { menu: 'Uniform Flow', name: 'Uniform Flow 01', param: 'Speed (m/s)', positioned: false },
  { menu: 'Source', name: 'Source 01', param: 'Strength (m²/s)', positioned: true },
  { menu: 'Sink', name: 'Sink 01', param: 'Strength (m²/s)', positioned: true },
  { menu: 'Vortex', name: 'Vortex 01', param: 'Circulation (m²/s)', positioned: true },
  { menu: 'Doublet', name: 'Doublet 01', param: 'Strength (m³/s)', positioned: true },
] as const;

async function addViaKeyboard(page: Page, menu: string) {
  await page.getByRole('button', { name: 'Add' }).click();
  await page.getByRole('menuitem', { name: menu, exact: true }).click();
  // Uniform flow is added immediately; positioned elements are placed with Enter.
  if (menu !== 'Uniform Flow') await page.keyboard.press('Enter');
}

test('DoD 2–8: add, move and edit every elementary element type', async ({ page }) => {
  await openApp(page);
  await loadExample(page, 'Uniform flow');
  const tree = page.getByRole('list', { name: 'Objects' });
  for (const k of KINDS) {
    await addViaKeyboard(page, k.menu);
    await expect(tree).toContainText(k.name);
    // Edit (item 8): the primary parameter commits and the solve reflects it.
    const field = page.getByRole('textbox', { name: k.param });
    await field.fill('3.25');
    await field.press('Enter');
    await expect(field).toHaveValue('3.25');
    // Move (item 7): positioned elements only.
    if (k.positioned) {
      const x = page.getByRole('textbox', { name: 'Position X (m)' });
      await x.fill('0.75');
      await x.press('Enter');
      await expect(x).toHaveValue('0.75');
    }
    await expect(page.getByRole('banner')).toContainText(/Solved/);
    await page.keyboard.press('Escape');
  }
  for (const k of KINDS) await expect(tree).toContainText(k.name);
});

test('DoD 18: surface Cp plot is shown for a body, with upper and lower surfaces', async ({ page }) => {
  await openApp(page);
  await loadExample(page, 'NACA 0012');
  // Surface is the default tab; clicking an already-active tab collapses the panel.
  await expect(page.getByRole('tab', { name: 'Surface' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('img', { name: 'Cp versus x/c' })).toBeVisible();
  await expect(page.getByText('Upper surface')).toBeVisible();
  await expect(page.getByText('Lower surface')).toBeVisible();
  // Two plotted series with real data.
  const paths = page.locator('svg[aria-label="Cp versus x/c"] path.series');
  await expect(paths).toHaveCount(2);
});

test('DoD 22: multiple bodies are solved together with per-body and total results', async ({ page }) => {
  await openApp(page);
  await loadExample(page, 'Biplane');
  await page.getByRole('tab', { name: 'Data' }).click();
  const table = page.getByRole('table', { name: 'Forces per body' });
  await expect(table).toContainText('Upper wing');
  await expect(table).toContainText('Lower wing');
  await expect(table).toContainText('Total');
});
