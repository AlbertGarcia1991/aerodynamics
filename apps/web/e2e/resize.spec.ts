/** Resizable panels and the bottom-panel chart layout. */
import { expect, test, type Page } from '@playwright/test';
import { loadExample, openApp } from './helpers';

async function dragBy(page: Page, name: string, dx: number, dy: number) {
  const h = page.getByRole('separator', { name });
  const b = (await h.boundingBox())!;
  const x = b.x + b.width / 2;
  const y = b.y + b.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx / 2, y + dy / 2, { steps: 4 });
  await page.mouse.move(x + dx, y + dy, { steps: 4 });
  await page.mouse.up();
}

const width = (page: Page, name: string) => page.getByRole('complementary', { name }).evaluate((el) => el.getBoundingClientRect().width);
const bottomHeight = (page: Page) => page.getByRole('region', { name: 'Analysis' }).evaluate((el) => el.getBoundingClientRect().height);

test('left, right and bottom panels resize by dragging their edge, and persist', async ({ page }) => {
  await openApp(page);
  const left0 = await width(page, 'Scene objects');
  await dragBy(page, 'Resize scene panel', 120, 0);
  expect(await width(page, 'Scene objects')).toBeCloseTo(left0 + 120, -1);

  const right0 = await width(page, 'Properties and results');
  await dragBy(page, 'Resize properties panel', -100, 0);
  expect(await width(page, 'Properties and results')).toBeCloseTo(right0 + 100, -1);

  const bottom0 = await bottomHeight(page);
  await dragBy(page, 'Resize analysis panel', 0, -90);
  expect(await bottomHeight(page)).toBeCloseTo(bottom0 + 90, -1);

  // Sizes survive a reload.
  await page.reload();
  await expect(page.getByRole('banner')).toContainText(/Solved/);
  expect(await width(page, 'Scene objects')).toBeCloseTo(left0 + 120, -1);
  expect(await bottomHeight(page)).toBeCloseTo(bottom0 + 90, -1);

  // Clamped: the canvas can never be squeezed away.
  await dragBy(page, 'Resize scene panel', 2000, 0);
  expect(await width(page, 'Scene objects')).toBeLessThanOrEqual(520);

  // Double-click restores the default.
  await page.getByRole('separator', { name: 'Resize scene panel' }).dblclick();
  expect(await width(page, 'Scene objects')).toBeCloseTo(260, -1);
});

test('separators resize from the keyboard', async ({ page }) => {
  await openApp(page);
  const h = page.getByRole('separator', { name: 'Resize analysis panel' });
  const before = Number(await h.getAttribute('aria-valuenow'));
  await h.focus();
  await page.keyboard.press('Shift+ArrowUp');
  await expect(h).toHaveAttribute('aria-valuenow', String(before + 64));
  await page.keyboard.press('Home');
  await expect(h).toHaveAttribute('aria-valuenow', '280');
});

test('surface plot is drawn at its real size and the side controls are not squashed', async ({ page }) => {
  await page.setViewportSize({ width: 1900, height: 900 });
  await openApp(page);
  await loadExample(page, 'NACA 0012');
  const svg = page.locator('svg[aria-label="Cp versus x/c"]');
  await expect(svg).toBeVisible();
  // viewBox matches the rendered box, i.e. no stretching of text or lines.
  const { vb, w, h } = await svg.evaluate((el) => ({
    vb: el.getAttribute('viewBox')!.split(' ').map(Number),
    w: el.getBoundingClientRect().width,
    h: el.getBoundingClientRect().height,
  }));
  expect(Math.abs(vb[2] - w)).toBeLessThan(2);
  expect(Math.abs(vb[3] - h)).toBeLessThan(2);
  // The quantity buttons keep their full height and readable labels.
  for (const name of ['Cp', 'V/U∞', 'Δp']) {
    const box = (await page.getByRole('radio', { name, exact: true }).boundingBox())!;
    expect(box.height, `${name} button height`).toBeGreaterThan(18);
  }
});
