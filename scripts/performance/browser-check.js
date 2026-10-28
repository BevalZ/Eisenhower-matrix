async (page) => {
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('http://127.0.0.1:1421/?count=1000');
  await page.locator('[data-task-id="1"]').waitFor();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.waitForFunction(() => !document.querySelector('.task-card.entering'));
  const layout = await page.evaluate(() => ({
    height: document.querySelector('.grid').clientHeight,
    scrollable: [...document.querySelectorAll('.q-body')].every((el) => el.scrollHeight > el.clientHeight),
  }));
  if (layout.height > 750 || !layout.scrollable) throw new Error(`Unbounded layout: ${JSON.stringify(layout)}`);
  const burst = await page.evaluate(async () => {
    let mutations = 0;
    const observer = new MutationObserver((records) => { mutations += records.length; });
    observer.observe(document.querySelector('.board'), { subtree: true, childList: true, attributes: true, characterData: true });
    const before = perfFixture.calls.filter((c) => c === 'get_tasks_delta').length;
    for (let i = 0; i < 100; i++) perfFixture.emit();
    await new Promise((resolve) => setTimeout(resolve, 100));
    observer.disconnect();
    return { reads: perfFixture.calls.filter((c) => c === 'get_tasks_delta').length - before, mutations };
  });
  if (burst.reads !== 1 || burst.mutations !== 0) throw new Error(JSON.stringify(burst));
  for (let i = 0; i < 4; i++) {
    await page.getByRole('button', { name: '统计', exact: true }).click();
    await page.getByRole('heading', { name: '统计概览' }).waitFor();
    await page.getByRole('button', { name: '看板', exact: true }).click();
    await page.locator('[data-task-id="1"]').waitFor();
  }
  const listeners = await page.evaluate(() => perfFixture.listeners.size);
  if (listeners !== 1) throw new Error(`Listener leak: ${listeners}`);
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.locator('.overlay').waitFor();
  await page.keyboard.press('Escape');
  await page.locator('.overlay').waitFor({ state: 'hidden' });
  await page.getByRole('button', { name: '+ 新建任务', exact: true }).click();
  await page.locator('.wizard-overlay').waitFor();
  await page.keyboard.press('Escape');
  await page.locator('.wizard-overlay').waitFor({ state: 'hidden' });
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
  await page.locator('[data-task-id="1"]').focus();
  await page.keyboard.press('4');
  await page.waitForFunction(() => perfFixture.tasks[0].quadrant === 4);
  await page.locator('[data-task-id="1"]').press('Enter');
  await page.getByRole('dialog').waitFor();
  await page.getByLabel('标题', { exact: true }).fill('性能回归编辑');
  // A longer description changes the card's height, which a windowed column has to re-measure.
  const shortCard = await page.locator('[data-task-id="1"]').boundingBox();
  await page.getByLabel('描述').fill('用于检查卡片高度变化后虚拟列表是否重新测量。'.repeat(4));
  await page.getByRole('button', { name: '保存', exact: true }).click();
  await page.getByRole('dialog').waitFor({ state: 'hidden' });
  await page.waitForFunction(() => perfFixture.tasks[0].title === '性能回归编辑');
  await page.waitForFunction(() => !document.querySelector('.task-card.entering'));
  const tallCard = await page.locator('[data-task-id="1"]').boundingBox();
  if (!(tallCard.height > shortCard.height)) {
    throw new Error(`Card did not grow: ${JSON.stringify({ before: shortCard.height, after: tallCard.height })}`);
  }

  // ---- Windowing: a 1,000-task board must not mount 1,000 cards ----
  const windowed = await page.evaluate(() => ({
    tasks: perfFixture.tasks.length,
    mounted: document.querySelectorAll('[data-task-id]').length,
    pads: document.querySelectorAll('.v-pad').length,
  }));
  if (windowed.mounted > 200 || windowed.pads < 2) throw new Error(`Not windowed: ${JSON.stringify(windowed)}`);

  // Scrolling to the end of a column reaches its last card (quadrant 4 ends at id 1000).
  const deep = await page.evaluate(async () => {
    const body = document.querySelectorAll('.q-body')[3];
    for (let i = 0; i < 40; i++) {
      body.scrollTop = body.scrollHeight;
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    }
    const ids = [...body.querySelectorAll('[data-task-id]')].map((el) => Number(el.dataset.taskId));
    return { top: ids[0], last: ids[ids.length - 1], mounted: document.querySelectorAll('[data-task-id]').length };
  });
  if (deep.last !== 1000 || deep.mounted > 200) throw new Error(`Deep scroll: ${JSON.stringify(deep)}`);

  // A card that was only reachable by scrolling can still be dragged out of the column.
  const deepCard = await page.locator('[data-task-id="1000"]').boundingBox();
  const other = await page.locator('.quadrant').nth(1).locator('.q-body').boundingBox();
  await page.mouse.move(deepCard.x + deepCard.width / 2, deepCard.y + 20);
  await page.mouse.down();
  await page.mouse.move(deepCard.x + deepCard.width / 2 - 20, deepCard.y + 20, { steps: 3 });
  await page.mouse.move(other.x + other.width / 2, other.y + 40, { steps: 10 });
  await page.mouse.up();
  await page.locator('.drag-ghost').waitFor({ state: 'hidden' });
  await page.waitForFunction(() => perfFixture.tasks[999].quadrant === 2);

  // A remote edit reaches the board through the incremental read alone: the row is patched in
  // place, without a full reload of the 1,000 tasks.
  const remote = await page.evaluate(async () => {
    const before = perfFixture.calls.filter((c) => c === 'get_tasks_delta').length;
    const card = document.querySelector('[data-task-id]');
    const id = Number(card.dataset.taskId);
    perfFixture.edit(id, { title: '远端改的标题' });
    perfFixture.emit();
    await new Promise((resolve) => setTimeout(resolve, 200));
    const after = perfFixture.calls.filter((c) => c === 'get_tasks_delta').length;
    return {
      reads: after - before,
      patched: document.querySelector(`[data-task-id="${id}"]`)?.textContent?.includes('远端改的标题') ?? false,
      mounted: document.querySelectorAll('[data-task-id]').length,
    };
  });
  if (remote.reads !== 1 || !remote.patched) throw new Error(`Incremental refresh missed the edit: ${JSON.stringify(remote)}`);

  await page.screenshot({ path: 'output/playwright/performance-board.png' });
  if (errors.length) throw new Error(JSON.stringify(errors));
  return {
    layout, burst, windowed, deep, remote, listenersAfterFourViewCycles: listeners,
    drag: 'passed', keyboard: 'passed', edit: 'passed', dialogs: 'passed', errors,
  };
}
