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
        await expect(page.locator('#overall')).toHaveText('100.0%');
        await expect(page.locator('#tree > .node')).toHaveAttribute('data-status', 'Running');
      }
    }
    await expect(page.locator('#tree > .node')).toHaveAttribute('data-status', scenario === 'cancel' ? 'Cancelled' : 'Succeeded');
    if (scenario === 'cancel') {
      await expect(page.locator('.node[data-status="NotRun"] > .node-heading > .node-name')).toHaveText('Encode WebP tiles');
      await expect(page.locator('#overall')).toHaveText('50.0%');
    } else {
      await expect(page.locator('#overall')).toHaveText('100.0%');
    }
    if (scenario === 'skip') await expect(page.locator('.node[data-status="Skipped"] > .node-heading > .node-name')).toHaveText('Apply optional watermark');
    if (scenario === 'diagnostics') {
      await expect(page.locator('#diagnostic-kind')).toHaveText('AbandonedPhase');
      await expect(page.locator('.node[data-status="Abandoned"] > .node-heading > .node-name')).toHaveText('Encode preview tiles');
    }
    if (scenario === 'smooth') await expect(page.locator('#smoothed-value')).toHaveText('100.0%');
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
  expect(await page.locator('#tree > .node > .track > span').first().getAttribute('style')).toBe(await page.locator('#smoothed-bar').getAttribute('style'));
  expect(await page.locator('#tree > .node > .track > span').first().getAttribute('style')).not.toBe(await page.locator('#recorded-bar').getAttribute('style'));
  await expect(page.locator('#overall-label')).toHaveText('What the caller displays');
  expect(await page.locator('#overall').textContent()).toBe(await page.locator('#smoothed-value').textContent());
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


test('scenario changes keep the layout still and work reports have no narration delays', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  for (const width of [390, 1440]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.goto('./');
    let height;
    for (const key of ['sequence', 'cancel', 'fork', 'skip', 'smooth', 'diagnostics']) {
      await page.locator(`button[data-scenario="${key}"]`).click();
      do {
        const current = await page.locator('.demo').evaluate(node => node.getBoundingClientRect().height);
        height ??= current;
        expect(current).toBe(height);
        if (!await page.getByRole('button', { name: 'Next event' }).isEnabled()) break;
        await page.getByRole('button', { name: 'Next event' }).click();
      } while (true);
    }
  }
  const traces = await (await page.request.get('traces.json')).json();
  for (const trace of Object.values(traces)) {
    for (let index = 1; index < trace.frames.length; index++) {
      expect(trace.frames[index].at_ms - trace.frames[index - 1].at_ms).toBeLessThanOrEqual(100);
    }
  }
});


test('optional unknown work reserves its share while known work advances', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('./');
  await page.locator('button[data-scenario="skip"]').click();
  const trace = (await (await page.request.get('traces.json')).json()).skip;
  const position = trace.frames.findIndex(frame => frame.snapshot.root.children[0].completed === '512');
  expect(position).toBeGreaterThan(0);
  const root = trace.frames[position].snapshot.root;
  expect(root.fraction).toBe(0.2);
  expect(root.unresolved_fraction).toBe(0.1);
  expect(root.children[1].fraction).toBeNull();
  await page.locator('#event-position').fill(String(position));
  await expect(page.locator('#overall')).toHaveText('20.0%');
  await expect(page.locator('#tree > .node > .track > .unknown-part')).toHaveAttribute('style', 'width: 10%;');
  await expect(page.locator('#tree > .node > .track > span').first()).toHaveAttribute('style', 'width: 20%;');
});


test('dev tools show actual uneven pace and count mismatches without losing handoff diagnostics', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('./');
  await page.locator('button[data-scenario="diagnostics"]').click();
  const trace = (await (await page.request.get('traces.json')).json()).diagnostics;
  expect(trace.findings.map(finding => finding.kind)).toEqual(['UnevenPace', 'CountMismatch', 'CountMismatch', 'AbandonedPhase']);
  const children = trace.frames.at(-1).snapshot.root.children;
  expect(children[0].completed).toBe('48');
  expect(children[0].total.count).toBe('64');
  expect(children[1].completed).toBe('80');
  expect(children[1].total.count).toBe('64');
  expect(children[1].overrun).toBe(true);
  for (let index = 0; index < trace.findings.length; index++) {
    const position = trace.frames.findIndex(frame => frame.finding === index);
    expect(position).toBeGreaterThan(0);
    await page.locator('#event-position').fill(String(position));
    await expect(page.locator('#diagnostic-kind')).toHaveText(trace.findings[index].kind);
    await expect(page.locator('#diagnostic-evidence')).toHaveText(trace.findings[index].evidence);
    await page.locator('#diagnostic-more summary').click();
    await expect(page.locator('#diagnostic-advice')).toHaveText(trace.findings[index].advice);
    await expect(page.locator('#diagnostic-advice')).toBeVisible();
    await page.locator('#diagnostic-more summary').click();
  }
});

test('every progress display keeps one decimal place', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('./');
  for (const scenario of ['sequence', 'cancel', 'fork', 'skip', 'smooth', 'diagnostics']) {
    await page.locator(`button[data-scenario="${scenario}"]`).click();
    do {
      await expect(page.locator('#overall')).toHaveText(/^\d+\.\d%$/);
      const details = await page.locator('.node-details').allTextContents();
      for (const text of details) expect(text).toMatch(/^(\d+\.\d%|Unknown) ·/);
      if (!await page.getByRole('button', { name: 'Next event' }).isEnabled()) break;
      await page.getByRole('button', { name: 'Next event' }).click();
    } while (true);
  }
});


test('parallel analysis shows fork ownership, independent reports and both joins', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto('./');
  await page.locator('[data-scenario="fork"]').click();
  const trace = (await (await page.request.get('traces.json')).json()).fork;
  const fork = page.locator('.node[data-execution="ForkJoin"]');
  await expect(fork.locator(':scope > .node-details')).toContainText('parallel branches');
  const branches = fork.locator(':scope > .node-children > .node');
  const boxes = await branches.evaluateAll(nodes => nodes.map(node => ({ left: node.getBoundingClientRect().left, top: node.getBoundingClientRect().top })));
  expect(boxes).toHaveLength(2);
  expect(boxes[0].top).toBe(boxes[1].top);
  expect(boxes[1].left).toBeGreaterThan(boxes[0].left);
  await page.getByRole('button', { name: 'Next event' }).click();
  await expect(page.locator('#event-code')).toContainText('let pixels_task = scope.spawn');
  await expect(page.locator('#event-code')).toContainText('let metadata_task = scope.spawn');
  await page.getByRole('button', { name: 'Next event' }).click();
  const profileReady = trace.frames[trace.events[2].frame].snapshot.root;
  expect(profileReady.fraction).toBe(0.3125);
  expect(profileReady.children[0].children[0].completed).toBe('48');
  expect(profileReady.children[0].children[1].outcome).toBe('Succeeded');
  expect(profileReady.children[0].status).toBe('Running');
  await page.getByRole('button', { name: 'Next event' }).click();
  await expect(page.locator('#event-code')).toContainText('pixels_task.join()');
  await expect(page.locator('#event-code')).toContainText('metadata_task.join()');
  await expect(page.locator('#event-code')).toContainText('match (histogram, profile)');
  await expect(fork).toHaveAttribute('data-status', 'Running');
  await page.getByRole('button', { name: 'Next event' }).click();
  await expect(page.locator('.node[data-status="Running"] > .node-heading > .node-name')).toHaveText(['Prepare a photo export', 'Write export manifest']);
  const activeFrames = trace.frames.filter(frame => frame.event === 1).map(frame => frame.snapshot.root.children[0].children);
  expect(activeFrames.some((children, index) => index > 0 && children[0].completed !== activeFrames[index - 1][0].completed && children[1].completed === activeFrames[index - 1][1].completed)).toBe(true);
});
