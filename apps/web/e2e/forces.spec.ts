/** Force arrows and element (Lagally) forces in the UI. */
import { expect, test } from '@playwright/test';
import { loadExample, openApp } from './helpers';

type Sol = { elements: { name: string; force: { x: number; y: number } }[]; bodies: { forces: { fx: number; fy: number } }[] };
const solution = (page: import('@playwright/test').Page) =>
  page.evaluate(() => (window as unknown as { __aeroflow: { useSolverStore: { getState(): { solution: Sol } } } }).__aeroflow.useSolverStore.getState().solution);

test('a vortex in a stream reports the Kutta–Joukowski force ρU∞|Γ|', async ({ page }) => {
  await openApp(page);
  await loadExample(page, 'Vortex in a freestream');
  await page.getByRole('button', { name: /^Vortex 01/ }).click();
  // ρ = 1.225, U∞ = 1, Γ = −3 (clockwise) → 3.675 N/m upwards.
  const fy = page.locator('dl.kv dt:has-text("Fy") + dd').first();
  await expect(fy).toHaveText('3.675');
  await expect(page.getByText(/Lagally force: what it takes to/)).toBeVisible();
  // The help link opens the explanation.
  await page.getByRole('button', { name: 'Why?' }).first().click();
  await expect(page.getByRole('dialog', { name: 'Help' })).toContainText('Lagally theorem');
});

test('a source near a cylinder and the cylinder pull on each other equally', async ({ page }) => {
  await openApp(page);
  await loadExample(page, 'Flow around a cylinder');
  await page.evaluate(() => {
    const w = window as unknown as { __aeroflow: { useSimulationStore: { getState(): { setConditions(c: { velocity: number }): void; addElement(k: string, p: { x: number; y: number }): string } } } };
    const st = w.__aeroflow.useSimulationStore.getState();
    st.setConditions({ velocity: 0 });
    st.addElement('source', { x: 2.2, y: 0.8 });
  });
  await expect.poll(async () => (await solution(page))?.elements.length ?? 0).toBe(1);
  const sol = await solution(page);
  const fe = sol.elements[0].force;
  const fb = { x: sol.bodies[0].forces.fx, y: sol.bodies[0].forces.fy };
  const rel = Math.hypot(fe.x + fb.x, fe.y + fb.y) / Math.hypot(fe.x, fe.y);
  // The balance converges at first order (Rust: 5.0 % at 60 panels, 1.24 % at
  // 240); this example uses 120 panels, observed 2.0 %.
  expect(rel).toBeLessThan(0.03);
  // Element forces appear in the Data tab.
  await page.getByRole('tab', { name: 'Data' }).click();
  await expect(page.getByRole('table', { name: 'Forces on elements' })).toContainText('Source 01');
});

test('the Forces toggle shows and hides the arrows', async ({ page }) => {
  await openApp(page);
  await loadExample(page, 'NACA 0012');
  const toggle = page.getByRole('toolbar', { name: 'Visualisation' }).getByRole('button', { name: 'Forces' });
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('o');
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
});
