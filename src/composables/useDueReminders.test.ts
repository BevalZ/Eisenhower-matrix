import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope, nextTick, ref } from "vue";
import { useDueReminders } from "./useDueReminders";
import type { Task } from "../types";

const hooks = vi.hoisted(() => ({ mounted: [] as (() => void)[], unmount: [] as (() => void)[], show: vi.fn() }));
vi.mock("vue", async (original) => ({
  ...await original<typeof import("vue")>(),
  onMounted: (callback: () => void) => hooks.mounted.push(callback),
  onBeforeUnmount: (callback: () => void) => hooks.unmount.push(callback),
}));
vi.mock("./useToast", () => ({ useToast: () => ({ show: hooks.show }) }));

let hidden = false;
let saved = "[]";
beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(1_000_000);
  hooks.mounted.length = 0;
  hooks.unmount.length = 0;
  hooks.show.mockClear();
  hidden = false;
  saved = "[]";
  const doc = new EventTarget();
  Object.defineProperty(doc, "hidden", { get: () => hidden });
  vi.stubGlobal("document", doc);
  vi.stubGlobal("localStorage", { getItem: () => saved, setItem: (_key: string, value: string) => { saved = value; } });
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

function mount(count = 1, ready = ref(true)) {
  const tasks = ref<Task[]>(Array.from({ length: count }, (_, id) => ({
    id, title: `Task ${id}`, description: "", quadrant: 2, priority: 50, importance_score: -1,
    urgency_score: -1, done: false, created_at: 1, completed_at: null, due_at: 2_000_000,
  })));
  const scope = effectScope();
  scope.run(() => useDueReminders(tasks, vi.fn(), ready));
  hooks.mounted.forEach((callback) => callback());
  return { tasks, unmount() { hooks.unmount.forEach((callback) => callback()); scope.stop(); } };
}

describe("deadline resource lifecycle", () => {
  it("retains deduplication for more than 500 active deadlines across remounts", () => {
    const first = mount(1000);
    expect(hooks.show).toHaveBeenCalledTimes(1000);
    first.unmount();
    const second = mount(1000);
    expect(hooks.show).toHaveBeenCalledTimes(1000);
    second.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("prunes completed and removed deadlines from persisted memory", async () => {
    const view = mount(3);
    view.tasks.value[0].done = true;
    view.tasks.value.splice(1, 1);
    await nextTick();
    expect(JSON.parse(saved)).toEqual(["2:2000000:soon"]);
    view.unmount();
  });

  it("keeps persisted reminders until the first successful task load", async () => {
    saved = '["0:2000000:soon"]';
    const ready = ref(false);
    const view = mount(0, ready);
    expect(saved).toBe('["0:2000000:soon"]');
    ready.value = true;
    await nextTick();
    expect(saved).toBe("[]");
    view.unmount();
  });

  it("stops hidden-window timers and checks elapsed deadlines on resume", () => {
    const view = mount();
    expect(vi.getTimerCount()).toBe(1);
    hidden = true;
    document.dispatchEvent(new Event("visibilitychange"));
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(2_000_000);
    expect(hooks.show).toHaveBeenCalledTimes(1);
    hidden = false;
    document.dispatchEvent(new Event("visibilitychange"));
    expect(hooks.show).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(1);
    view.unmount();
    document.dispatchEvent(new Event("visibilitychange"));
    expect(vi.getTimerCount()).toBe(0);
  });
});
