async (page) => {
  await page.addInitScript(() => {
    const params = new URLSearchParams(location.search);
    const callbacks = new Map();
    const listeners = new Map();
    const calls = [];
    let nextId = 1;
    const tasks = Array.from({ length: Number(params.get('count') ?? 1000) }, (_, i) => ({
      id: i + 1, title: `测试任务 ${i + 1}`, description: i % 3 ? '' : '用于检查拖拽、排序及界面响应',
      quadrant: i % 4 + 1, priority: 100 - i % 100, importance_score: -1, urgency_score: -1,
      done: false, created_at: 1, completed_at: null, due_at: null,
    }));
    const settings = { api_key_configured: false, theme: 'light', webdav_configured: false };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: params.get('ball') ? 'floating-ball' : 'main' }, currentWebview: { label: 'main' } },
      transformCallback(callback) { const id = nextId++; callbacks.set(id, callback); return id; },
      async invoke(command, args) {
        calls.push(command);
        if (command === 'get_tasks') return tasks.map((task) => ({ ...task }));
        if (command === 'get_settings') return { ...settings };
        if (command === 'platform') return 'windows';
        if (command === 'get_webdav_config') return { url: '', username: '', has_password: false };
        if (command === 'plugin:event|listen') { listeners.set(args.handler, args.event); return args.handler; }
        if (command === 'plugin:event|unlisten') { listeners.delete(args.eventId); callbacks.delete(args.eventId); return; }
        if (command === 'reorder_tasks') { for (const move of args.moves) Object.assign(tasks.find((t) => t.id === move.id), move); return; }
        if (command === 'update_task') { Object.assign(tasks.find((t) => t.id === args.input.id), args.input); return; }
        if (command === 'toggle_task_done') { const task = tasks.find((t) => t.id === args.id); task.done = !task.done; return; }
        if (command === 'get_stats') return { total: tasks.length, done: 0, pending: tasks.length, completion_rate: 0,
          by_quadrant: Object.fromEntries([1,2,3,4].map((q) => [q, { total: tasks.filter((t) => t.quadrant === q).length, done: 0 }])),
          week_by_quadrant: {1:0,2:0,3:0,4:0}, recent_completed: [], overdue: 0 };
        if (command === 'get_learning_stats') return { total_feedback: 0, corrections: 0, importance_bias: 0, urgency_bias: 0 };
        return null;
      },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.perfFixture = { calls, tasks, listeners, emit() {
      for (const [id, event] of listeners) callbacks.get(id)?.({ event, id, payload: null });
    }};
  });
  await page.setViewportSize({ width: 1100, height: 750 });
  await page.goto('http://127.0.0.1:1421/?count=1000');
  await page.locator('[data-task-id]').first().waitFor();
  console.log(await page.evaluate(() => ({ cards: document.querySelectorAll('[data-task-id]').length, listeners: perfFixture.listeners.size })));
}
