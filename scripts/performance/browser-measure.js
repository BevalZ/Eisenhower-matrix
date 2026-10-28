/**
 * Numbers for docs/performance.md: how much of a large board is actually mounted, and what a
 * drag on it costs. Run after browser-setup.js (it installs the Tauri fixture).
 *
 * The current build is served on 1421; the pre-windowing baseline on 1422 is measured too when
 * it is running, and skipped otherwise.
 */
async (page) => {
  const COUNT = 10_000;

  async function measure(port) {
    const started = Date.now();
    await page.goto(`http://127.0.0.1:${port}/?count=${COUNT}`);
    await page.locator('[data-task-id]').first().waitFor();
    const firstCardMs = Date.now() - started;
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.waitForFunction(() => !document.querySelector('.task-card.entering'));
    const mounted = await page.evaluate(() => ({
      tasks: perfFixture.tasks.length,
      cards: document.querySelectorAll('[data-task-id]').length,
      elements: document.querySelectorAll('.board *').length,
      listHeight: Math.round(document.querySelector('.q-list').getBoundingClientRect().height),
      viewport: document.querySelector('.q-body').clientHeight,
    }));

    await page.evaluate(() => {
      window.__mutations = 0;
      const observer = new MutationObserver((records) => { window.__mutations += records.length; });
      observer.observe(document.querySelector('.board'), { subtree: true, childList: true, attributes: true, characterData: true });
    });
    const card = await page.locator('[data-task-id]').first().boundingBox();
    const body = await page.locator('.q-body').first().boundingBox();
    await page.mouse.move(card.x + card.width / 2, card.y + 20);
    await page.mouse.down();
    await page.mouse.move(card.x + card.width / 2 - 20, card.y + 20, { steps: 3 });
    await page.locator('.drag-ghost').waitFor();
    const slots = 12;
    for (let i = 0; i < slots; i++) {
      await page.mouse.move(body.x + body.width / 2, body.y + 40 + i * 24, { steps: 2 });
      await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    }
    await page.mouse.up();
    await page.locator('.drag-ghost').waitFor({ state: 'hidden' });
    const drag = await page.evaluate(() => ({
      mutations: window.__mutations,
      cards: document.querySelectorAll('[data-task-id]').length,
    }));

    // Scrolling a long column: windowed lists mount and drop cards while the list moves.
    const scroll = await page.evaluate(async () => {
      window.__scrollMutations = 0;
      const observer = new MutationObserver((records) => { window.__scrollMutations += records.length; });
      const body = document.querySelectorAll('.q-body')[0];
      observer.observe(body, { subtree: true, childList: true, attributes: true, characterData: true });
      const steps = 20;
      const mark = performance.now();
      for (let i = 0; i < steps; i++) {
        body.scrollTop += body.scrollHeight / steps;
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      }
      const ms = performance.now() - mark;
      observer.disconnect();
      return { steps, ms: +(ms / steps).toFixed(1), mutations: window.__scrollMutations,
        cards: document.querySelectorAll('[data-task-id]').length,
        atEnd: body.scrollTop + body.clientHeight >= body.scrollHeight - 2 };
    });

    return {
      firstCardMs,
      mounted,
      drag: { ...drag, slots, mutationsPerSlot: +(drag.mutations / slots).toFixed(1) },
      scroll,
    };
  }

  const current = await measure(1421);
  let baseline = null;
  try {
    baseline = await measure(1422);
  } catch (error) {
    baseline = { unavailable: String(error).split('\n')[0] };
  }
  return { current, baseline };
}
