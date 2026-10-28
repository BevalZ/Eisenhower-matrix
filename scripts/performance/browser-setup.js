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

    // Mirror the backend's incremental reads: every write takes a stamp above the previous one,
    // and a reader with a current cursor only gets rows written after it.
    let generation = 1;
    let watermark = 0;
    const stamps = new Map();
    const removed = new Set();
    const bump = (id) => { watermark += 1; stamps.set(id, watermark); };
    const snapshot = () => ({
      generation, watermark, full: true,
      tasks: tasks.filter((t) => !removed.has(t.id)).map((t) => ({ ...t })),
      removed: [],
    });
    function delta(args) {
      const mark = args?.watermark;
      if (args?.generation !== generation || mark == null) return snapshot();
      return {
        generation, watermark, full: false,
        tasks: tasks.filter((t) => !removed.has(t.id) && (stamps.get(t.id) ?? 0) > mark).map((t) => ({ ...t })),
        removed: [...removed].filter((id) => (stamps.get(id) ?? 0) > mark),
      };
    }

    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: params.get('ball') ? 'floating-ball' : 'main' }, currentWebview: { label: 'main' } },
      transformCallback(callback) { const id = nextId++; callbacks.set(id, callback); return id; },
      async invoke(command, args) {
        calls.push(command);
        if (command === 'get_tasks_delta') return delta(args ?? {});
        if (command === 'get_tasks') return tasks.map((task) => ({ ...task }));
        if (command === 'get_settings') return { ...settings };
        if (command === 'platform') return 'windows';
        if (command === 'get_webdav_config') return { url: '', username: '', has_password: false };
        if (command === 'plugin:event|listen') { listeners.set(args.handler, args.event); return args.handler; }
        if (command === 'plugin:event|unlisten') { listeners.delete(args.eventId); callbacks.delete(args.eventId); return; }
        if (command === 'reorder_tasks') { for (const move of args.moves) { Object.assign(tasks.find((t) => t.id === move.id), move); bump(move.id); } return; }
        if (command === 'update_task') { Object.assign(tasks.find((t) => t.id === args.input.id), args.input); bump(args.input.id); return; }
        if (command === 'toggle_task_done') { const task = tasks.find((t) => t.id === args.id); task.done = !task.done; bump(task.id); return; }
        if (command === 'get_stats') return { total: tasks.length, done: 0, pending: tasks.length, completion_rate: 0,
          by_quadrant: Object.fromEntries([1,2,3,4].map((q) => [q, { total: tasks.filter((t) => t.quadrant === q).length, done: 0 }])),
          week_by_quadrant: {1:0,2:0,3:0,4:0}, recent_completed: [], overdue: 0 };
        if (command === 'get_learning_stats') return { total_feedback: 0, corrections: 0, importance_bias: 0, urgency_bias: 0 };
        return null;
      },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.perfFixture = { calls, tasks, listeners, stamps,
      /** Simulate an edit arriving from another window (as the native sync does). */
      edit(id, patch) { Object.assign(tasks.find((t) => t.id === id), patch); bump(id); },
      emit() {
        for (const [id, event] of listeners) callbacks.get(id)?.({ event, id, payload: null });
      } };
  });
  await page.setViewportSize({ width: 1100, height: 750 });
  await page.goto('http://127.0.0.1:1421/?count=1000');
  await page.locator('[data-task-id]').first().waitFor();
  console.log(await page.evaluate(() => ({
    tasks: perfFixture.tasks.length,
    mounted: document.querySelectorAll('[data-task-id]').length,
    listeners: perfFixture.listeners.size,
  })));
}
