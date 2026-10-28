/**
 * The other half of the windowing check: a board below WINDOW_FROM cards per column must keep
 * rendering every card, with no spacers and no scrolling behaviour changes.
 * Run after browser-setup.js, which installs the Tauri fixture.
 */
async (page) => {
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('http://127.0.0.1:1421/?count=120');
  await page.locator('[data-task-id]').first().waitFor();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.waitForFunction(() => !document.querySelector('.task-card.entering'));

  const full = await page.evaluate(() => ({
    tasks: perfFixture.tasks.length,
    mounted: document.querySelectorAll('[data-task-id]').length,
    pads: document.querySelectorAll('.v-pad').length,
    last: Number(document.querySelectorAll('[data-task-id]')[document.querySelectorAll('[data-task-id]').length - 1].dataset.taskId),
  }));
  if (full.mounted !== full.tasks || full.pads !== 0) throw new Error(`Small board was windowed: ${JSON.stringify(full)}`);

  // The plain drop-slot path still works with every card mounted.
  const source = await page.locator('[data-task-id="1"]').boundingBox();
  const target = await page.locator('.quadrant').nth(1).locator('.q-body').boundingBox();
  await page.mouse.move(source.x + source.width / 2, source.y + 20);
  await page.mouse.down();
  await page.mouse.move(source.x + source.width / 2 - 20, source.y + 20, { steps: 3 });
  await page.locator('.drag-ghost').waitFor();
  await page.mouse.move(target.x + target.width / 2, target.y + 35, { steps: 8 });
  await page.mouse.up();
  await page.locator('.drag-ghost').waitFor({ state: 'hidden' });
  await page.waitForFunction(() => perfFixture.tasks[0].quadrant === 2);

  await page.screenshot({ path: 'output/playwright/performance-board-small.png' });
  if (errors.length) throw new Error(JSON.stringify(errors));
  return { full, drag: 'passed', errors };
}
