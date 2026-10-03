import { expect, type Page } from '@playwright/test';

/** Open the app with the welcome tour already dismissed and wait for the solver. */
export async function openApp(page: Page, opts: { theme?: 'light' | 'dark' } = {}): Promise<void> {
  // Runs before every navigation, so the theme must come from the caller —
  // setting localStorage and reloading would be overwritten here.
  await page.addInitScript((theme) => {
    localStorage.setItem('aeroflow.seen-welcome', '1');
    localStorage.setItem('aeroflow.theme', theme);
  }, opts.theme ?? 'light');
  await page.goto('/');
  await expect(page.getByText('Loading the WebAssembly solver')).toBeHidden({ timeout: 30_000 });
  await expect(page.getByRole('banner')).toContainText(/Solved/);
}

export async function loadExample(page: Page, title: string): Promise<void> {
  await page.getByRole('button', { name: 'Examples' }).click();
  await page.getByRole('button', { name: new RegExp('^' + title) }).click();
  await expect(page.getByRole('banner')).toContainText(/Solved/);
}

/** Read the lift coefficient shown for the selected body. */
export async function readCL(page: Page): Promise<number> {
  const dd = page.locator('dl.kv dt:has-text("CL") + dd').first();
  const text = await dd.textContent();
  return Number(text);
}

/** Assert the resolved theme, so a theme test cannot silently run in the other mode. */
export async function expectTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
  await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
}
