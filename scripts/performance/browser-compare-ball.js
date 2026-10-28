async (page) => {
  const cdp = await page.context().newCDPSession(page);
  const results = [];
  for (const [name, port] of [['before', 1422], ['after', 1421]]) {
    await page.goto(`http://127.0.0.1:${port}/?ball=1&count=10000`);
    await page.locator('.ball-window').waitFor();
    await cdp.send('HeapProfiler.collectGarbage');
    const initial = await cdp.send('Runtime.getHeapUsage');
    const snapshot = await page.evaluate(async () => {
      for (let i = 0; i < 100; i++) perfFixture.emit();
      await new Promise((resolve) => setTimeout(resolve, 100));
      return { listeners: perfFixture.listeners.size,
        taskReads: perfFixture.calls.filter((c) => c === 'get_tasks' || c === 'get_tasks_delta').length };
    });
    await cdp.send('HeapProfiler.collectGarbage');
    const synced = await cdp.send('Runtime.getHeapUsage');
    results.push({ name, ...snapshot, initialJSHeap: initial.usedSize, afterSyncJSHeap: synced.usedSize });
  }
  // Preserve the Android fallback even if an old native config labels it as a ball.
  await cdp.send('Network.setUserAgentOverride', { userAgent: 'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/130.0 Mobile Safari/537.36' });
  await page.goto('http://127.0.0.1:1421/?ball=1&count=8');
  await page.locator('.board').waitFor();
  if (await page.locator('[data-task-id]').count() !== 8) throw new Error('Mobile fallback failed');
  await cdp.detach();
  return { floatingBall: results, mobileWithLegacyWindowLabel: 'passed' };
}
