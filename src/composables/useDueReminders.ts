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
    // check() prunes deleted/completed tasks and old deadlines before persisting.
    localStorage.setItem(STORE_KEY, JSON.stringify([...seen]));
  } catch {
    /* storage unavailable */
  }
}

/**
 * A clock that ticks every minute, plus one in-app reminder per task when its
 * deadline comes within 48 hours and again when it passes.
 */
export function useDueReminders(tasks: Ref<Task[]>, move: (task: Task, q: Quadrant) => void, ready: Ref<boolean>) {
  const now = ref(Date.now());
  const { show } = useToast();
  const seen = loadSeen();
  let timer: ReturnType<typeof setInterval> | undefined;

  function check() {
    if (document.hidden || !ready.value) return;
    now.value = Date.now();
    let changed = false;
    const activeKeys = new Set<string>();
    for (const task of tasks.value) {
      if (task.done || task.due_at == null) continue;
      activeKeys.add(`${task.id}:${task.due_at}:soon`);
      activeKeys.add(`${task.id}:${task.due_at}:overdue`);
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
    for (const key of seen) {
      if (!activeKeys.has(key)) { seen.delete(key); changed = true; }
    }
    if (changed) saveSeen(seen);
  }

  function updateTimer() {
    clearInterval(timer);
    if (!document.hidden) {
      check();
      timer = setInterval(check, 60_000);
    }
  }

  onMounted(() => {
    updateTimer();
    document.addEventListener("visibilitychange", updateTimer);
  });
  onBeforeUnmount(() => {
    clearInterval(timer);
    document.removeEventListener("visibilitychange", updateTimer);
  });
  // New or edited deadlines are checked right away, not on the next tick. Only tasks with a
  // deadline take part in the signature: on a board with thousands of undated tasks, rebuilding
  // a string per task on every change costs more than the check itself.
  watch(
    () => {
      if (!ready.value) return "pending";
      let signature = "";
      for (const task of tasks.value) {
        if (task.due_at == null) continue;
        signature += `${task.id}:${task.due_at}:${task.done ? 1 : 0};`;
      }
      return signature;
    },
    check,
  );

  return { now };
}
