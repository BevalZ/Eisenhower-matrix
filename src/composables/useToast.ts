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
  toasts.value = [...toasts.value.slice(-3), { id, message, kind: opts.kind ?? "info", action: opts.action }];
  const duration = opts.duration ?? (opts.action ? 6000 : 3500);
  timers.set(id, setTimeout(() => dismiss(id), duration));
  return id;
}

function showError(prefix: string, err: unknown) {
  const detail = err instanceof Error ? err.message : String(err);
  show(`${prefix}：${detail}`, { kind: "error", duration: 6000 });
}

export function useToast() {
  return { toasts, show, showError, dismiss };
}
