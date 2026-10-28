import { describe, expect, it, vi } from "vitest";
import { computed, ref } from "vue";
import { reconcileTasks } from "./taskSnapshot";
import type { Task } from "./types";

export function makeTask(id: number): Task {
  return {
    id, title: `task ${id}`, description: "", quadrant: 1, priority: 50,
    importance_score: 3, urgency_score: 3, done: false, created_at: 1,
    completed_at: null, due_at: null,
  };
}

describe("task snapshot reconciliation", () => {
  it("retains all identities and avoids reactive invalidation on a no-op refresh", () => {
    const tasks = ref(Array.from({ length: 1000 }, (_, i) => makeTask(i)));
    const group = vi.fn(() => tasks.value.filter((t) => t.quadrant === 1));
    const quadrant = computed(group);
    const previous = quadrant.value;
    const array = tasks.value;
    tasks.value = reconcileTasks(tasks.value, tasks.value.map((t) => ({ ...t })));
    expect(tasks.value).toBe(array);
    expect(quadrant.value).toBe(previous);
    expect(group).toHaveBeenCalledTimes(1);
  });

  it("updates existing task objects, including cleared deadlines, without replacing siblings", () => {
    const tasks = ref([makeTask(1), { ...makeTask(2), due_at: 20 }]);
    const first = tasks.value[0];
    const edited = tasks.value[1];
    const title = computed(() => edited.title);
    expect(title.value).toBe("task 2");
    tasks.value = reconcileTasks(tasks.value, [makeTask(1), { ...makeTask(2), title: "edited", due_at: null }]);
    expect(tasks.value[0]).toBe(first);
    expect(tasks.value[1]).toBe(edited);
    expect(title.value).toBe("edited");
    expect(edited.due_at).toBeNull();
  });

  it("reconciles insertion, deletion and backend order", () => {
    const current = [makeTask(1), makeTask(2), makeTask(3)];
    const next = reconcileTasks(current, [makeTask(3), makeTask(4), makeTask(1)]);
    expect(next.map((t) => t.id)).toEqual([3, 4, 1]);
    expect(next[0]).toBe(current[2]);
    expect(next[2]).toBe(current[0]);
  });
});
