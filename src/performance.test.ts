import { expect, it, vi } from "vitest";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { reconcileTasks } from "./taskSnapshot";
import { compareTasks, lowerBound } from "./ordering";
import type { Task } from "./types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock("./composables/useToast", () => ({ useToast: () => ({ showError: vi.fn(), show: vi.fn() }) }));

function fixture(count: number): Task[] {
  return Array.from({ length: count }, (_, id) => ({ id, title: `Task ${id}`, description: "benchmark",
    quadrant: 1, priority: id % 100, importance_score: -1, urgency_score: -1,
    done: false, created_at: 1, completed_at: null, due_at: null }));
}

it("keeps a 10,000-task unchanged refresh stable and measures before/after work", () => {
  const data = fixture(10_000);
  function measure(reconcile: boolean) {
    const tasks = ref(data.map((task) => ({ ...task })));
    let sorts = 0;
    const sorted = computed(() => { sorts++; return [...tasks.value].sort(compareTasks); });
    const first = sorted.value;
    const start = performance.now();
    for (let i = 0; i < 10; i++) {
      const incoming = data.map((task) => ({ ...task }));
      tasks.value = reconcile ? reconcileTasks(tasks.value, incoming) : incoming;
      expect(sorted.value).toHaveLength(data.length);
    }
    return { ms: +(performance.now() - start).toFixed(2), sorts: sorts - 1, retained: sorted.value === first };
  }
  const before = measure(false);
  const after = measure(true);
  expect(after.sorts).toBe(0);
  expect(after.retained).toBe(true);
  console.info("10 x 10,000-task no-op refresh (ms are informational):", { before, after });
});

it("bounds batch reorder lookup work linearly at 5,000 tasks", async () => {
  const store = (await import("./composables/useTasks")).useTasks();
  let idReads = 0;
  store.tasks.value = fixture(5_000).map((task) => ({ ...task, get id() { idReads++; return task.id; } }));
  const start = performance.now();
  await store.reorderTasks(Array.from({ length: 5_000 }, (_, id) => ({ id, quadrant: 1, priority: 50 })));
  const ms = +(performance.now() - start).toFixed(2);
  expect(invoke).toHaveBeenCalledWith("reorder_tasks", expect.anything());
  expect(idReads).toBeLessThanOrEqual(10_000);
  console.info("5,000-task batch reorder:", { ms, idReads, previousLinearScanReads: 5_000 * 5_001 / 2 });
});

it("bounds geometry reads independently of total DOM size", () => {
  const rows = [100, 1000, 10_000].map((cards) => {
    let reads = 0;
    const target = Math.floor(cards * 0.7);
    expect(lowerBound(cards, (i) => { reads++; return i >= target; })).toBe(target);
    expect(reads).toBeLessThanOrEqual(Math.ceil(Math.log2(cards + 1)));
    return { cards, previousGeometryReads: cards, geometryReads: reads };
  });
  console.info("Drag geometry budget:", rows);
});
