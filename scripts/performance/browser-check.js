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
    const before = perfFixture.calls.filter((c) => c === 'get_tasks').length;
    for (let i = 0; i < 100; i++) perfFixture.emit();
    await new Promise((resolve) => setTimeout(resolve, 100));
    observer.disconnect();
    return { reads: perfFixture.calls.filter((c) => c === 'get_tasks').length - before, mutations };
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
  await page.getByRole('button', { name: '保存', exact: true }).click();
  await page.getByRole('dialog').waitFor({ state: 'hidden' });
  await page.waitForFunction(() => perfFixture.tasks[0].title === '性能回归编辑');
  await page.screenshot({ path: 'output/playwright/performance-board.png' });
  if (errors.length) throw new Error(JSON.stringify(errors));
  return { layout, burst, listenersAfterFourViewCycles: listeners, drag: 'passed', keyboard: 'passed', edit: 'passed', dialogs: 'passed', errors };
}
