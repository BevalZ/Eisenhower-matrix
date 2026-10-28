import { describe, expect, it } from "vitest";
import { applyTaskDelta } from "./taskDelta";
import type { Task } from "./types";

function makeTask(id: number): Task {
  return {
    id, title: `task ${id}`, description: "", quadrant: 1, priority: 50,
    importance_score: 3, urgency_score: 3, done: false, created_at: 1,
    completed_at: null, due_at: null,
  };
}

describe("incremental task refresh", () => {
  it("keeps the array and every object when a delta reports no changes", () => {
    const current = Array.from({ length: 500 }, (_, i) => makeTask(i));
    const array = current;
    const next = applyTaskDelta(current, { generation: 1, watermark: 9, full: false, tasks: [], removed: [] });
    expect(next).toBe(array);
  });

  it("rewrites only the changed rows and reuses the object of an edited task", () => {
    const current = [makeTask(1), makeTask(2), makeTask(3)];
    const edited = current[1];
    const sibling = current[2];
    const next = applyTaskDelta(current, {
      generation: 1, watermark: 10, full: false,
      tasks: [{ ...makeTask(2), title: "edited" }], removed: [],
    });
    expect(next).not.toBe(current);
    expect(next).toHaveLength(3);
    expect(next[1]).toBe(edited);
    expect(edited.title).toBe("edited");
    expect(next[2]).toBe(sibling);
  });

  it("adds new rows, drops removed ids and ignores ids that are already gone", () => {
    const current = [makeTask(1), makeTask(2)];
    const next = applyTaskDelta(current, {
      generation: 1, watermark: 11, full: false,
      tasks: [makeTask(3)], removed: [1, 999],
    });
    expect(next.map((t) => t.id).sort()).toEqual([2, 3]);
  });

  it("routes a full answer through reconciliation so identities survive", () => {
    const current = [makeTask(1), makeTask(2)];
    const first = current[0];
    const next = applyTaskDelta(current, {
      generation: 2, watermark: 12, full: true, tasks: [makeTask(1), makeTask(2)], removed: [],
    });
    expect(next).toBe(current);
    expect(next[0]).toBe(first);
  });
});
