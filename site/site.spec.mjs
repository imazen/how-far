import { test, expect } from '@playwright/test';

test('trace player preserves completion, cancellation, skipping and join ownership', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  await page.goto('./');
  await expect(page.locator('#event-count')).toHaveText('STEP 1 / 7');
  for (const scenario of ['sequence', 'cancel', 'fork', 'skip', 'smooth', 'diagnostics']) {
    await page.locator(`button[data-scenario="${scenario}"]`).click();
    if (scenario === 'fork') {
      await page.getByRole('button', { name: 'Next event' }).click();
      await page.getByRole('button', { name: 'Next event' }).click();
      await expect(page.locator('.node[data-status="Succeeded"] > .node-heading > .node-name')).toHaveText('Read color profile');
      await expect(page.locator('#tree > .node')).toHaveAttribute('data-status', 'Running');
    }
    while (await page.getByRole('button', { name: 'Next event' }).isEnabled()) {
      await page.getByRole('button', { name: 'Next event' }).click();
      if (scenario === 'sequence' && await page.locator('#event-title').textContent() === 'All stages finished') {
        await expect(page.locator('#overall')).toHaveText('100%');
        await expect(page.locator('#tree > .node')).toHaveAttribute('data-status', 'Running');
      }
    }
    await expect(page.locator('#tree > .node')).toHaveAttribute('data-status', scenario === 'cancel' ? 'Cancelled' : 'Succeeded');
    if (scenario === 'cancel') {
      await expect(page.locator('.node[data-status="NotRun"] > .node-heading > .node-name')).toHaveText('Encode WebP tiles');
      await expect(page.locator('#overall')).toHaveText('50%');
    } else {
      await expect(page.locator('#overall')).toHaveText('100%');
    }
    if (scenario === 'skip') await expect(page.locator('.node[data-status="Skipped"] > .node-heading > .node-name')).toHaveText('Apply optional watermark');
    if (scenario === 'diagnostics') {
      await expect(page.locator('#diagnostic-kind')).toHaveText('AbandonedPhase');
      await expect(page.locator('.node[data-status="Abandoned"] > .node-heading > .node-name')).toHaveText('Encode preview tiles');
    }
    if (scenario === 'smooth') await expect(page.locator('#smoothed-value')).toHaveText('100%');
  }
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Pause', exact: true }).click();
  await page.locator('#event-position').focus();
  await page.keyboard.press('End');
  await expect(page.locator('#tree > .node')).toHaveAttribute('data-status', 'Succeeded');
  expect(errors).toEqual([]);
});

test('autoplay cycles, manual controls stop cycling, and smoothing has real intermediate estimates', async ({ page }) => {
  await page.clock.install();
  await page.goto('./');
  await page.locator('.demo').scrollIntoViewIfNeeded();
  await expect(page.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
  await page.clock.runFor(12000);
  await expect(page.locator('button[data-scenario="cancel"]')).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Smoothing', exact: true }).click();
  await page.clock.runFor(1700);
  const recorded = Number((await page.locator('#recorded-value').textContent()).replace('%', ''));
  const shown = Number((await page.locator('#smoothed-value').textContent()).replace('%', ''));
  expect(shown).toBeGreaterThan(recorded);
  await page.getByRole('button', { name: 'Pause', exact: true }).click();
  const position = await page.locator('#event-position').inputValue();
  await page.clock.runFor(15000);
  await expect(page.locator('#event-position')).toHaveValue(position);
  await expect(page.locator('button[data-scenario="smooth"]')).toHaveAttribute('aria-pressed', 'true');
});

for (const width of [390, 1440]) {
  test(`guide navigation and layout at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 1000 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    for (const path of ['', 'getting-started/', 'library/', 'tracking/', 'diagnostics/', 'integration/', 'testing/', 'design/', 'validation/']) {
      const response = await page.goto('./' + path);
      expect(response.status()).toBe(200);
      await expect(page.locator('main h1')).toBeVisible();
      if (path === 'getting-started/') await expect(page.locator('pre.giallo span[style]').first()).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      // Check every local link and anchor, including links rewritten from source Markdown.
      const links = await page.locator('a[href]').evaluateAll(nodes => nodes.map(n => n.href).filter(href => href.startsWith(location.origin)));
      for (const link of new Set(links)) {
        const target = new URL(link);
        const response = await page.request.get(target.pathname);
        expect(response.status(), link).toBe(200);
        if (target.hash) {
          const html = await response.text();
          expect(html, link).toContain(`id="${decodeURIComponent(target.hash.slice(1))}"`);
        }
      }
    }
  });
}
