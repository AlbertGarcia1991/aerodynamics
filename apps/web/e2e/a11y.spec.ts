/**
 * Accessibility (PRD §54, WCAG 2.2 AA where practical): automated axe scans of
 * the main states plus a keyboard-only workflow.
 */
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { expectTheme, loadExample, openApp } from './helpers';

async function scan(page: Page, label: string) {
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
    .analyze();
  const summary = results.violations.map((v) => `${v.id} (${v.impact}): ${v.nodes.length}× — ${v.nodes.slice(0, 3).map((n) => n.target.join(' ')).join(' | ')}`);
  expect(summary, `${label}\n${summary.join('\n')}`).toEqual([]);
}

test.describe('accessibility', () => {
  test('main workspace (light) has no WCAG A/AA violations', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('button', { name: /^NACA 0012/ }).click();
    await scan(page, 'light, body selected');
  });

  test('main workspace (dark) has no WCAG A/AA violations', async ({ page }) => {
    await openApp(page, { theme: 'dark' });
    await expectTheme(page, 'dark');
    await scan(page, 'dark');
  });

  test('dialogs and help drawer have no WCAG A/AA violations', async ({ page }) => {
    await openApp(page);
    await page.getByRole('button', { name: 'Examples' }).click();
    await scan(page, 'examples dialog');
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'Add' }).click();
    await page.getByRole('menuitem', { name: 'Import coordinates' }).click();
    await scan(page, 'import dialog');
    await page.keyboard.press('Escape');
    await page.getByRole('tab', { name: 'Polar' }).click();
    await scan(page, 'polar tab');
    await page.getByRole('tab', { name: 'Diagnostics' }).click();
    await scan(page, 'diagnostics tab');
  });

  test('keyboard-only: add a source, nudge it, edit it, delete it', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'Uniform flow');
    await page.getByRole('button', { name: 'Add' }).focus();
    await page.keyboard.press('Enter');
    await page.getByRole('menuitem', { name: 'Source', exact: true }).focus();
    await page.keyboard.press('Enter');
    // Placement is armed; Enter drops it at the view centre — no pointer needed.
    await page.keyboard.press('Enter');
    const tree = page.getByRole('list', { name: 'Objects' });
    await expect(tree).toContainText('Source 01');
    const x = page.getByRole('textbox', { name: 'Position X (m)' });
    const x0 = Number(await x.inputValue());
    await page.locator('body').focus();
    await page.keyboard.press('ArrowRight');
    await expect.poll(async () => Number(await x.inputValue())).toBeGreaterThan(x0);
    await page.keyboard.press('Delete');
    await expect(tree).not.toContainText('Source 01');
  });

  test('visible focus ring on interactive controls', async ({ page }) => {
    await openApp(page);
    await page.keyboard.press('Tab');
    await page.keyboard.press('Tab');
    const shadow = await page.evaluate(() => getComputedStyle(document.activeElement!).boxShadow);
    expect(shadow).not.toBe('none');
  });
});

test.describe('responsive layout (PRD §38)', () => {
  for (const [label, width, height] of [['tablet', 820, 1100], ['phone', 390, 844]] as const) {
    test(`${label}: canvas usable, side panels collapsible, no horizontal scroll`, async ({ page }) => {
      await page.setViewportSize({ width, height });
      await openApp(page);
      // Compact mode starts with both side panels closed so the canvas dominates.
      await expect(page.getByRole('complementary', { name: 'Scene objects' })).toBeHidden();
      const canvas = await page.locator('.canvas-stack').boundingBox();
      expect(canvas!.width).toBeGreaterThan(width * 0.9);
      // Panels open as overlays from the toolbar.
      await page.getByRole('button', { name: 'Toggle scene panel' }).click();
      await expect(page.getByRole('complementary', { name: 'Scene objects' })).toBeVisible();
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
      expect(overflow).toBeLessThanOrEqual(0);
    });
  }
});
