import { expect, test, type Page } from '@playwright/test';
import { openApp, readCL } from './helpers';

/** PRD2 §88 interactive regression and §116 acceptance flows, against the real WASM solver. */

type Stores = {
  useSimulationStore: { getState(): { revision: number; scene: any; undo(): void } };
  useSolverStore: { getState(): { solvedRevision: number; solution: any; status: string } };
  useViewportStore: { getState(): { center: { x: number; y: number }; scale: number; width: number; height: number } };
};
const stores = (page: Page) => ({
  state: () =>
    page.evaluate(() => {
      const a = (window as unknown as { __aeroflow: Stores }).__aeroflow;
      const sim = a.useSimulationStore.getState();
      const sol = a.useSolverStore.getState();
      return {
        revision: sim.revision,
        solvedRevision: sol.solvedRevision,
        status: sol.status,
        body: sim.scene.bodies[0] ?? null,
        cp: (sol.solution?.bodies[0]?.surface ?? []).map((p: { cp: number | null }) => p.cp ?? 0) as number[],
        lift: sol.solution?.total.lift as number | undefined,
      };
    }),
});

/** The app opens on an example cylinder; start from an empty scene so body 0 is the one under test. */
async function emptyScene(page: Page) {
  await page.evaluate(() => {
    const a = (window as unknown as { __aeroflow: Stores }).__aeroflow;
    (a.useSimulationStore.getState() as any).update((s: any) => {
      s.bodies = [];
      s.elements = [];
    });
  });
  await settled(page);
}

async function createBezierBody(page: Page, template: string) {
  await page.getByRole('button', { name: 'Add' }).click();
  await page.getByRole('menuitem', { name: 'Create geometry' }).click();
  await page.getByLabel('Bézier template').selectOption({ label: template });
  await page.getByRole('button', { name: 'Create', exact: true }).click();
  await settled(page);
}

/** Wait until the displayed solution belongs to the current geometry revision. */
async function settled(page: Page) {
  await expect
    .poll(async () => {
      const s = await stores(page).state();
      return s.solvedRevision === s.revision && s.status !== 'running';
    })
    .toBe(true);
  await page.waitForTimeout(350); // let the idle full-resolution pass land too
  await expect
    .poll(async () => {
      const s = await stores(page).state();
      return s.solvedRevision === s.revision;
    })
    .toBe(true);
}

/** Screen position of the body's node with the largest local y (the upper-surface crest). */
async function crestNode(page: Page) {
  return page.evaluate(() => {
    const a = (window as unknown as { __aeroflow: Stores }).__aeroflow;
    const body = a.useSimulationStore.getState().scene.bodies[0];
    const vp = a.useViewportStore.getState();
    const node = [...body.geometry.nodes].sort((p: any, q: any) => q.position.y - p.position.y)[0];
    const wx = body.position.x + node.position.x * body.scale;
    const wy = body.position.y + node.position.y * body.scale;
    const box = document.querySelector('.canvas-stack')!.getBoundingClientRect();
    return {
      id: node.id as string,
      x: box.left + vp.width / 2 + (wx - vp.center.x) * vp.scale,
      y: box.top + vp.height / 2 - (wy - vp.center.y) * vp.scale,
      scale: vp.scale,
    };
  });
}

test.describe('Bézier geometry editor (PRD2)', () => {
  test('a symmetric Bézier airfoil gives CL ≈ 0 at zero incidence', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await createBezierBody(page, 'Symmetric airfoil (NACA 0012)');
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Custom body 01');
    expect(Math.abs(await readCL(page))).toBeLessThan(0.02);
  });

  test('a cambered Bézier airfoil matches thin-airfoil theory (CL ≈ 0.25 at α = 0)', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await createBezierBody(page, 'Airfoil (NACA 2412)');
    const cl = await readCL(page);
    expect(cl).toBeGreaterThan(0.2);
    expect(cl).toBeLessThan(0.3);
  });

  test('a Bézier circle reproduces the analytical cylinder: Cp_min = −3, no lift', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await createBezierBody(page, 'Circle');
    const s = await stores(page).state();
    expect(Math.min(...s.cp)).toBeGreaterThan(-3.08);
    expect(Math.min(...s.cp)).toBeLessThan(-2.92);
    expect(Math.abs(s.lift ?? 1)).toBeLessThan(0.05);
  });

  test('dragging a node updates revision, Cp and lift, and is one undo step (§88)', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await createBezierBody(page, 'Airfoil (NACA 2412)');
    const before = await stores(page).state();
    const clBefore = await readCL(page);

    // Creating a Bézier body already switches to the Node tool.
    await expect(page.getByRole('button', { name: 'Nodes', exact: true })).toHaveAttribute('aria-pressed', 'true');
    const node = await crestNode(page);
    // Select, then drag the crest up by ~0.05 chord.
    await page.mouse.click(node.x, node.y);
    await page.mouse.move(node.x, node.y);
    await page.mouse.down();
    await page.mouse.move(node.x, node.y - 0.02 * node.scale, { steps: 4 });
    await page.mouse.move(node.x, node.y - 0.05 * node.scale, { steps: 4 });
    await page.mouse.up();
    await settled(page);

    const after = await stores(page).state();
    expect(after.revision).toBeGreaterThan(before.revision);
    expect(after.solvedRevision).toBe(after.revision); // the solver result matches the geometry shown
    const moved = after.body.geometry.nodes.find((n: any) => n.id === node.id);
    const orig = before.body.geometry.nodes.find((n: any) => n.id === node.id);
    expect(moved.position.y).toBeGreaterThan(orig.position.y + 0.03);
    expect(Math.max(...after.cp.map((v, i) => Math.abs(v - (before.cp[i] ?? v))))).toBeGreaterThan(1e-3);
    expect(Math.abs((await readCL(page)) - clBefore)).toBeGreaterThan(1e-3);

    // One gesture, one undo entry.
    await page.keyboard.press('Control+z');
    await settled(page);
    const undone = await stores(page).state();
    expect(undone.body.geometry.nodes.find((n: any) => n.id === node.id).position.y).toBeCloseTo(orig.position.y, 9);
  });

  test('a self-intersecting shape is reported, excluded from the solve, and never crashes the app (§50, §63)', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await createBezierBody(page, 'Symmetric airfoil (NACA 0012)');
    // Creating a Bézier body already switches to the Node tool.
    await expect(page.getByRole('button', { name: 'Nodes', exact: true })).toHaveAttribute('aria-pressed', 'true');
    const node = await crestNode(page);
    await page.mouse.click(node.x, node.y);
    await page.mouse.move(node.x, node.y);
    await page.mouse.down();
    await page.mouse.move(node.x, node.y + 0.25 * node.scale, { steps: 6 }); // well through the lower surface (half-thickness 0.06 m)
    await page.mouse.up();

    await expect(page.locator('.status-badges')).toContainText('Invalid geometry');
    await expect(page.getByRole('banner')).toBeVisible();
    await page.keyboard.press('Control+z');
    await expect(page.locator('.status-badges', { hasText: 'Invalid geometry' })).toHaveCount(0);
    await settled(page);
    expect(Math.abs(await readCL(page))).toBeLessThan(0.02);
  });

  test('a strongly cambered shape splits into one upper and one lower surface (no Cp zigzag)', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await createBezierBody(page, 'Airfoil (NACA 2412)');
    // Arch the shape so both surfaces lie on the same side of the straight LE→TE line.
    await page.evaluate(() => {
      const sim = (window as any).__aeroflow.useSimulationStore.getState();
      sim.updateBody(sim.scene.bodies[0].id, (b: any) => ({
        ...b,
        geometry: { ...b.geometry, nodes: b.geometry.nodes.map((n: any) => ({ ...n, position: { x: n.position.x, y: n.position.y + 1.6 * (0.25 - n.position.x * n.position.x) } })) },
      }));
    });
    await settled(page);
    const { upper, lower, upperX } = await page.evaluate(() => {
      const s = (window as any).__aeroflow.useSolverStore.getState().solution.bodies[0].surface;
      const up = s.filter((p: any) => p.surface === 'upper');
      return { upper: up.length, lower: s.length - up.length, upperX: up.map((p: any) => p.xOverC) as number[] };
    });
    expect(Math.abs(upper - lower)).toBeLessThan(0.2 * (upper + lower));
    // Each surface runs one way along the chord; interleaved surfaces would reverse direction constantly.
    let reversals = 0;
    for (let i = 2; i < upperX.length; i++) if ((upperX[i] - upperX[i - 1]) * (upperX[i - 1] - upperX[i - 2]) < 0) reversals++;
    expect(reversals).toBeLessThan(3);
  });

  test('the pen draws a closed body that is solved (§116.2–8)', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await page.keyboard.press('p');
    const box = (await page.locator('.canvas-stack').boundingBox())!;
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    const pts = [[-90, 60], [90, 60], [90, -60], [-90, -60]];
    for (const [dx, dy] of pts) await page.mouse.click(cx + dx, cy + dy);
    await expect(page.getByRole('list', { name: 'Objects' })).toContainText('Custom body 01');
    await expect(page.locator('.status-badges')).toContainText('Open path');
    await page.mouse.click(cx + pts[0][0], cy + pts[0][1]); // click the first node to close
    await expect(page.locator('.status-badges', { hasText: 'Open path' })).toHaveCount(0);
    await settled(page);
    const s = await stores(page).state();
    expect(s.body.geometry.closed).toBe(true);
    expect(s.body.geometry.nodes).toHaveLength(4);
    await expect(page.getByRole('banner')).toContainText(/Solved/);
  });

  test('the panel mesh toggle and numeric node editor work', async ({ page }) => {
    await openApp(page);
    await emptyScene(page);
    await createBezierBody(page, 'Circle');
    await page.getByRole('button', { name: 'Panels', exact: true }).click();
    // Creating a Bézier body already switches to the Node tool.
    await expect(page.getByRole('button', { name: 'Nodes', exact: true })).toHaveAttribute('aria-pressed', 'true');
    const node = await crestNode(page);
    await page.mouse.click(node.x, node.y);
    // The node form sits in the Geometry section, above the body's own Transform fields.
    const y = page.getByRole('textbox', { name: 'Position Y (m)' }).first();
    await expect(y).toBeVisible();
    await y.fill('0.6');
    await y.press('Enter');
    await settled(page);
    const s = await stores(page).state();
    expect(Math.max(...s.body.geometry.nodes.map((n: any) => n.position.y))).toBeCloseTo(0.6, 3);
  });
});
