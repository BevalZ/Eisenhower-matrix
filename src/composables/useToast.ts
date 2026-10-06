import { ref } from "vue";

export interface ToastAction {
  label: string;
  run: () => void | Promise<void>;
}

export interface Toast {
  id: number;
  message: string;
  kind: "info" | "error";
  action?: ToastAction;
}

const toasts = ref<Toast[]>([]);
const timers = new Map<number, ReturnType<typeof setTimeout>>();
let nextId = 1;

function dismiss(id: number) {
  clearTimeout(timers.get(id));
  timers.delete(id);
  toasts.value = toasts.value.filter((t) => t.id !== id);
}

function show(
  message: string,
  opts: { kind?: Toast["kind"]; action?: ToastAction; duration?: number } = {},
): number {
  const id = nextId++;
  // Only the newest four stay on screen; drop the evicted one's timer with it.
  const kept = toasts.value.slice(-3);
  for (const evicted of toasts.value.slice(0, -3)) {
    clearTimeout(timers.get(evicted.id));
    timers.delete(evicted.id);
  }
  toasts.value = [...kept, { id, message, kind: opts.kind ?? "info", action: opts.action }];
  const duration = opts.duration ?? (opts.action ? 6000 : 3500);
  timers.set(id, setTimeout(() => dismiss(id), duration));
  return id;
}

/** Text for a rejected `invoke` (Tauri rejects with a plain string, not an Error). */
export function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function showError(prefix: string, err: unknown) {
  show(`${prefix}：${errorText(err)}`, { kind: "error", duration: 6000 });
}

export function useToast() {
  return { toasts, show, showError, dismiss };
}
