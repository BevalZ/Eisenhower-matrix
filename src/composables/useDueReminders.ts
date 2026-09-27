import { onBeforeUnmount, onMounted, ref, watch, type Ref } from "vue";
import type { Quadrant, Task } from "../types";
import { QUADRANT_META } from "../types";
import { dueState, urgentTarget } from "../due";
import { useToast } from "./useToast";

const STORE_KEY = "due-reminded";

/** Keys like "uid-or-id:due_at:soon" that were already announced. */
function loadSeen(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(STORE_KEY) ?? "[]"));
  } catch {
    return new Set();
  }
}

function saveSeen(seen: Set<string>) {
  try {
    // Keep the list bounded; old entries only matter for tasks still on the board.
    localStorage.setItem(STORE_KEY, JSON.stringify([...seen].slice(-500)));
  } catch {
    /* storage unavailable */
  }
}

/**
 * A clock that ticks every minute, plus one in-app reminder per task when its
 * deadline comes within 48 hours and again when it passes.
 */
export function useDueReminders(tasks: Ref<Task[]>, move: (task: Task, q: Quadrant) => void) {
  const now = ref(Date.now());
  const { show } = useToast();
  const seen = loadSeen();
  let timer: ReturnType<typeof setInterval> | undefined;

  function check() {
    now.value = Date.now();
    let changed = false;
    for (const task of tasks.value) {
      if (task.done || task.due_at == null) continue;
      const state = dueState(task.due_at, now.value);
      if (state === "later") continue;
      const key = `${task.id}:${task.due_at}:${state}`;
      if (seen.has(key)) continue;
      seen.add(key);
      changed = true;
      const target = urgentTarget(task, now.value);
      show(state === "overdue" ? `「${task.title}」已过截止时间` : `「${task.title}」将在 48 小时内截止`, {
        kind: state === "overdue" ? "error" : "info",
        duration: 10_000,
        action: target ? { label: `移到${QUADRANT_META[target].name}`, run: () => move(task, target) } : undefined,
      });
    }
    if (changed) saveSeen(seen);
  }

  onMounted(() => {
    check();
    timer = setInterval(check, 60_000);
  });
  onBeforeUnmount(() => clearInterval(timer));
  // New or edited deadlines are checked right away, not on the next tick.
  watch(() => tasks.value.map((t) => `${t.id}:${t.due_at}:${t.done}`).join(), check);

  return { now };
}
