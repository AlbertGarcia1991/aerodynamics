import { expect, test } from '@playwright/test';
import { loadExample, openApp, readCL } from './helpers';

test.describe('core flows (PRD §63)', () => {
  test('launches, solves the default scene and shows a legend', async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole('group', { name: /Legend/ })).toBeVisible();
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Freestream');
    // A freshly loaded scene must not count as unsaved work.
    await page.waitForTimeout(500);
    const dirty = await page.evaluate(() => (window as unknown as { __aeroflow: { useSimulationStore: { getState(): { dirty: boolean } } } }).__aeroflow.useSimulationStore.getState().dirty);
    expect(dirty).toBe(false);
  });

  test('create source → move it → change strength → add vortex', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'Uniform flow');
    await page.getByRole('button', { name: 'Add' }).click();
    await page.getByRole('menuitem', { name: 'Source', exact: true }).click();
    const canvas = page.locator('.canvas-stack');
    const box = (await canvas.boundingBox())!;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Source 01');

    // Drag the source 120 px to the right; its x must increase.
    const xBefore = Number(await page.getByRole('textbox', { name: 'Position X (m)' }).inputValue());
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2, { steps: 5 });
    await page.mouse.move(box.x + box.width / 2 + 120, box.y + box.height / 2, { steps: 5 });
    await page.mouse.up();
    const xAfter = Number(await page.getByRole('textbox', { name: 'Position X (m)' }).inputValue());
    expect(xAfter).toBeGreaterThan(xBefore + 0.1);

    // Change strength numerically.
    const strength = page.getByRole('textbox', { name: 'Strength (m²/s)' });
    await strength.fill('7.5');
    await strength.press('Enter');
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Λ = 7.50');

    // Add a vortex at the view centre with Alt+click.
    await page.getByRole('button', { name: 'Add' }).click();
    await page.getByRole('menuitem', { name: 'Vortex', exact: true }).click({ modifiers: ['Alt'] });
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Vortex 01');

    // Undo removes the vortex again.
    await page.keyboard.press('Control+z');
    await expect(page.getByRole('list', { name: 'Objects' })).not.toContainText('Vortex 01');
  });

  test('solves a NACA 0012 airfoil and reports lift, drag and coefficients', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('button', { name: /^NACA 0012/ }).click();
    const cl = await readCL(page);
    expect(cl).toBeGreaterThan(0.57);
    expect(cl).toBeLessThan(0.63);
    await expect(page.getByText(/Kutta–Joukowski lift agree/)).toBeVisible();
    await expect(page.getByText(/d'Alembert/)).toBeVisible();
    await expect(page.locator('.pill--kutta')).toHaveText('kutta');
  });

  test('every example that shows streamlines actually produces them', async ({ page }) => {
    // Regression: a body covering the view centre used to yield zero streamlines.
    await openApp(page);
    for (const title of ['Flow around a cylinder', 'NACA 0012', 'Source and sink', 'Biplane']) {
      await loadExample(page, title);
      await expect
        .poll(
          () =>
            page.evaluate(() => {
              const w = window as unknown as { __aeroflow: { useSolverStore: { getState(): { streamlines: { count: number } | null } } } };
              return w.__aeroflow.useSolverStore.getState().streamlines?.count ?? 0;
            }),
          { message: `${title}: streamline count` },
        )
        .toBeGreaterThan(8);
    }
  });

  test('switches fields and the legend follows', async ({ page }) => {
    await openApp(page);
    const toolbar = page.getByRole('toolbar', { name: 'Visualisation' });
    await toolbar.getByRole('button', { name: 'Pressure coefficient' }).click();
    await expect(page.getByRole('group', { name: /Legend/ })).toContainText('Pressure coefficient');
    await page.keyboard.press('5');
    await expect(page.getByRole('group', { name: /Legend/ })).toContainText('Vorticity');
    await toolbar.getByRole('button', { name: 'None' }).click();
    await expect(page.getByRole('group', { name: /Legend/ })).toBeHidden();
  });

  test('imports a geometry file through the dialog', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'Uniform flow');
    await page.getByRole('button', { name: 'Add' }).click();
    await page.getByRole('menuitem', { name: 'Import coordinates' }).click();
    // A coarse NACA-like contour, Selig order.
    const rows = ['x,y'];
    const n = 40;
    for (let i = 0; i <= n; i++) {
      const b = (Math.PI * i) / n;
      const x = 0.5 * (1 + Math.cos(b));
      const t = 5 * 0.12 * (0.2969 * Math.sqrt(x) - 0.126 * x - 0.3516 * x * x + 0.2843 * x ** 3 - 0.1036 * x ** 4);
      rows.push(`${x.toFixed(5)},${t.toFixed(5)}`);
    }
    for (let i = 1; i < n; i++) {
      const b = (Math.PI * i) / n;
      const x = 0.5 * (1 - Math.cos(b));
      const t = 5 * 0.12 * (0.2969 * Math.sqrt(x) - 0.126 * x - 0.3516 * x * x + 0.2843 * x ** 3 - 0.1036 * x ** 4);
      rows.push(`${x.toFixed(5)},${(-t).toFixed(5)}`);
    }
    await page.getByLabel('Coordinate text').fill(rows.join('\n'));
    await expect(page.getByText(/No problems found/)).toBeVisible();
    await page.getByRole('button', { name: 'Import', exact: true }).click();
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('points');
    await expect(page.getByRole('banner')).toContainText(/Solved/);
  });

  test('rejects a self-intersecting contour with an actionable message', async ({ page }) => {
    await openApp(page);
    await page.getByRole('button', { name: 'Add' }).click();
    await page.getByRole('menuitem', { name: 'Import coordinates' }).click();
    await page.getByLabel('Coordinate text').fill('0,0\n1,1\n1,0\n0,1\n');
    await expect(page.getByText(/crosses another segment/)).toBeVisible();
    await expect(page.getByRole('button', { name: 'Import', exact: true })).toBeDisabled();
  });

  test('saves and reloads a simulation', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'Source and sink');
    await page.getByRole('button', { name: 'File', exact: true }).click();
    const download = page.waitForEvent('download');
    await page.getByRole('menuitem', { name: /^Save/ }).click();
    const file = await download;
    const path = await file.path();
    expect(file.suggestedFilename()).toMatch(/\.aeroflow\.json$/);

    await page.getByRole('button', { name: 'File', exact: true }).click();
    await page.getByRole('menuitem', { name: 'New simulation' }).click();
    await expect(page.getByRole('list', { name: 'Objects' })).not.toContainText('Source 01');

    await page.getByRole('button', { name: 'File', exact: true }).click();
    const chooser = page.waitForEvent('filechooser');
    await page.getByRole('menuitem', { name: /Open/ }).click();
    await (await chooser).setFiles(path!);
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Source 01');
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Sink 01');
  });

  test('exports force results as CSV', async ({ page }) => {
    await openApp(page);
    await page.getByRole('tab', { name: 'Data' }).click();
    const download = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Forces CSV' }).click();
    const file = await download;
    expect(file.suggestedFilename()).toMatch(/_forces\.csv$/);
  });

  test('runs an angle-of-attack sweep to completion', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('tab', { name: 'Polar' }).click();
    await page.getByRole('button', { name: 'Run sweep' }).click();
    await expect(page.getByText(/α \[deg\]/).first()).toBeVisible({ timeout: 30_000 });
    await expect(page.getByRole('button', { name: 'Sweep CSV' })).toBeVisible();
    await expect(page.getByText(/Cancelled after/)).toBeHidden();
  });

  test('cancels a long sweep in the worker (PRD §60)', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('button', { name: /^NACA 0012/ }).click();
    const count = page.getByRole('textbox', { name: 'Panel count' });
    await count.fill('1000');
    await count.press('Enter');
    await expect(page.getByRole('banner')).toContainText(/1000 panels/, { timeout: 30_000 });
    await page.getByRole('tab', { name: 'Polar' }).click();
    await page.getByLabel('Sweep points').fill('201');
    await page.getByRole('button', { name: 'Run sweep' }).click();
    await page.getByRole('button', { name: /^Cancel/ }).click();
    const note = page.getByText(/Cancelled after \d+ points/);
    await expect(note).toBeVisible();
    const done = Number((await note.textContent())!.match(/(\d+) points/)![1]);
    expect(done).toBeLessThan(201);
    // The worker is free again: a normal solve still completes.
    await page.keyboard.press('Escape');
    await expect(page.getByRole('button', { name: 'Run sweep' })).toBeEnabled();
  });

  test('a circulation sweep on a cylinder follows Kutta–Joukowski', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'Flow around a cylinder');
    await page.getByRole('tab', { name: 'Polar' }).click();
    await page.getByLabel('Sweep parameter').selectOption('bodyCirculation');
    await page.getByLabel('Sweep points').fill('5');
    await page.getByRole('button', { name: 'Run sweep' }).click();
    await expect(page.getByText(/Γ \[m²\/s\]/).first()).toBeVisible({ timeout: 30_000 });
    // Check the physics through the store: L = −ρU∞Γ at every point.
    const pts = await page.evaluate(() => {
      const w = window as unknown as { __aeroflow: { useSolverStore: { getState(): { sweep: { points: { value: number; bodies: { lift: number }[] }[] } } } } };
      return w.__aeroflow.useSolverStore.getState().sweep.points.map((p) => [p.value, p.bodies[0].lift]);
    });
    expect(pts).toHaveLength(5);
    for (const [gamma, lift] of pts) {
      const expected = -1.225 * 1 * gamma;
      expect(Math.abs(lift - expected)).toBeLessThan(0.01 * Math.abs(expected) + 1e-6);
    }
  });

  test('a custom moment reference point changes Cm but not CL', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('button', { name: /^NACA 0012/ }).click();
    const cmCell = page.locator('dl.kv dt:has-text("Cm (ref. point)") + dd').first();
    const cm0 = Number(await cmCell.textContent());
    const cl0 = await readCL(page);
    await page.getByLabel('Custom moment reference point').check();
    const xr = page.getByRole('textbox', { name: 'Reference x (body) (m)' });
    await xr.fill('0');
    await xr.press('Enter');
    await expect(cmCell).not.toHaveText(String(cm0));
    // About the leading edge, positive lift aft of it pitches nose-down: Cm < 0.
    await expect.poll(async () => Number(await cmCell.textContent())).toBeLessThan(-0.1);
    expect(await readCL(page)).toBeCloseTo(cl0, 4);
  });

  test('stays responsive while solving a large body', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('button', { name: /^NACA 0012/ }).click();
    const count = page.getByRole('textbox', { name: 'Panel count' });
    await count.fill('900');
    await count.press('Enter');
    // The UI thread must keep painting: a trivial DOM interaction completes quickly.
    const t0 = Date.now();
    await page.getByRole('button', { name: 'Examples' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    expect(Date.now() - t0).toBeLessThan(2000);
    await page.keyboard.press('Escape');
    await expect(page.getByRole('banner')).toContainText(/Solved/, { timeout: 30_000 });
  });
});

test.describe('polar chart (CD ≈ 0 must read as ≈ 0)', () => {
  test('the drag polar x-axis is ranged around the data, not forced to [0, 1]', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('tab', { name: 'Polar' }).click();
    await page.getByLabel('Sweep points').fill('9');
    await page.getByRole('button', { name: 'Run sweep' }).click();
    const polar = page.locator('svg[aria-label="CL versus CD (inviscid residual)"]');
    await expect(polar).toBeVisible({ timeout: 30_000 });
    // x tick labels sit on the bottom axis row; all must lie within ±0.02.
    const ticks = await polar.evaluate((svg) => {
      const h = svg.getBoundingClientRect().height;
      return Array.from(svg.querySelectorAll('g.axis text'))
        .filter((t) => Number(t.getAttribute('y')) > h - 30 && Number(t.getAttribute('y')) < h - 10)
        .map((t) => Number(t.textContent));
    });
    expect(ticks.length).toBeGreaterThan(2);
    for (const t of ticks) expect(Math.abs(t)).toBeLessThanOrEqual(0.02);
    await expect(page.getByText(/CD ≈ 0 is the correct inviscid result/)).toBeVisible();
  });

  test('a circulation sweep on an airfoil warns that the Kutta condition is overridden', async ({ page }) => {
    await openApp(page);
    await loadExample(page, 'NACA 0012');
    await page.getByRole('tab', { name: 'Polar' }).click();
    await page.getByLabel('Sweep parameter').selectOption('bodyCirculation');
    await expect(page.getByText(/Prescribing Γ overrides the Kutta condition/)).toBeVisible();
    // A cylinder has no sharp edge: no warning.
    await loadExample(page, 'Flow around a cylinder');
    await page.getByLabel('Sweep parameter').selectOption('bodyCirculation');
    await expect(page.getByText(/Prescribing Γ overrides the Kutta condition/)).toBeHidden();
  });
});
