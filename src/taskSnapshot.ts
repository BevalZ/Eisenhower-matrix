import type { Task } from "./types";

// Compile-time coverage: adding a Task field requires including it in reconciliation.
const fields = Object.keys({
  id: true, title: true, description: true, quadrant: true, priority: true,
  importance_score: true, urgency_score: true, done: true, created_at: true,
  completed_at: true, due_at: true,
} satisfies Record<keyof Task, true>) as (keyof Task)[];

/** Preserve reactive task objects (and the array on a no-op refresh). */
export function reconcileTasks(current: Task[], incoming: Task[]): Task[] {
  const byId = new Map(current.map((task) => [task.id, task]));
  let sameOrder = current.length === incoming.length;
  const next = incoming.map((task, index) => {
    const existing = byId.get(task.id);
    const result = existing ?? task;
    if (existing && fields.some((key) => existing[key] !== task[key])) Object.assign(existing, task);
    if (current[index] !== result) sameOrder = false;
    return result;
  });
  return sameOrder ? current : next;
}
