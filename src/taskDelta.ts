import type { Task } from "./types";
import { fieldsDiffer, reconcileTasks } from "./taskSnapshot";

/**
 * Answer to an incremental read (`get_tasks_delta`).
 *
 * `full: true` means the backend could not describe the changes from a cursor and sent the
 * whole board instead. Otherwise `tasks` only holds the rows written since the cursor and
 * `removed` the ids of tasks that were deleted, so a steady-state refresh moves a handful of
 * rows over IPC instead of the entire table.
 */
export interface TaskDelta {
  generation: number;
  watermark: number;
  full: boolean;
  tasks: Task[];
  removed: number[];
}

/** Where the next incremental read resumes from. */
export interface TaskCursor {
  generation: number;
  watermark: number;
}

/**
 * Fold a delta into the board.
 *
 * Untouched tasks keep their objects (and their Vue reactivity) so only cards whose fields
 * really changed re-render, and the current array is handed back untouched when nothing moved
 * at all — the same guarantee the full-snapshot reconciliation gives, without re-sorting.
 */
export function applyTaskDelta(current: Task[], delta: TaskDelta): Task[] {
  if (delta.full) return reconcileTasks(current, delta.tasks);
  if (!delta.tasks.length && !delta.removed.length) return current;

  const byId = new Map(current.map((task) => [task.id, task]));
  let changed = false;
  for (const task of delta.tasks) {
    const existing = byId.get(task.id);
    if (!existing) {
      byId.set(task.id, task);
      changed = true;
    } else if (fieldsDiffer(existing, task)) {
      Object.assign(existing, task);
      changed = true;
    }
  }
  for (const id of delta.removed) if (byId.delete(id)) changed = true;
  return changed ? [...byId.values()] : current;
}
