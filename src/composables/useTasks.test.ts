import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { Task } from "../types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./useToast", () => ({ useToast: () => ({ showError: vi.fn(), show: vi.fn() }) }));

function task(id: number): Task {
  return { id, title: `Task ${id}`, description: "", quadrant: 1, priority: 50,
    importance_score: 3, urgency_score: 3, done: false, created_at: 1,
    completed_at: null, due_at: null };
}
const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
beforeEach(() => { vi.resetModules(); vi.mocked(invoke).mockReset(); });

describe("task store", () => {
  it("serializes writes while keeping optimistic updates immediate", async () => {
    const store = (await import("./useTasks")).useTasks();
    store.tasks.value = [task(1)];
    let firstDone!: () => void;
    vi.mocked(invoke).mockReturnValueOnce(new Promise((resolve) => { firstDone = () => resolve(undefined); }))
      .mockResolvedValue(undefined);
    const first = store.toggleTaskDone(1);
    expect(store.tasks.value[0].done).toBe(true);
    const second = store.toggleTaskDone(1);
    expect(store.tasks.value[0].done).toBe(false);
    await flush();
    expect(invoke).toHaveBeenCalledTimes(1);
    firstDone();
    await Promise.all([first, second]);
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("rolls back every move on failure and permits a subsequent write", async () => {
    const store = (await import("./useTasks")).useTasks();
    store.tasks.value = [task(1), task(2)];
    vi.mocked(invoke).mockRejectedValueOnce(new Error("disk full")).mockResolvedValue(undefined);
    await store.reorderTasks([{ id: 1, quadrant: 2, priority: 10 }, { id: 2, quadrant: 3, priority: 20 }]);
    expect(store.tasks.value.map((t) => [t.quadrant, t.priority])).toEqual([[1, 50], [1, 50]]);
    await store.toggleTaskDone(1);
    expect(store.tasks.value[0].done).toBe(true);
  });

  it("does not reload under a drag, and coalesces requests when the hold ends", async () => {
    const store = (await import("./useTasks")).useTasks();
    vi.mocked(invoke).mockResolvedValue([task(1)]);
    const release = store.holdRemoteRefresh();
    const requests = Array.from({ length: 100 }, () => store.loadTasks({ quiet: true }));
    await flush();
    expect(invoke).not.toHaveBeenCalled();
    release();
    await Promise.all(requests);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(store.tasks.value).toHaveLength(1);
  });
});
