import type { Quadrant, Task } from "./types";

const HOUR = 3_600_000;
/** Within this window a deadline counts as urgent (matches the AI's "24 to 48 hours"). */
export const SOON_MS = 48 * HOUR;

export type DueState = "overdue" | "soon" | "later";

export function dueState(dueAt: number, now: number): DueState {
  if (dueAt < now) return "overdue";
  return dueAt - now <= SOON_MS ? "soon" : "later";
}

/** Short label for a card, e.g. "已逾期", "今天 18:00", "明天 09:30", "10-03 12:00". */
export function dueLabel(dueAt: number, now: number): string {
  const d = new Date(dueAt);
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (dueAt < now) return "已逾期";
  const days = calendarDaysBetween(new Date(now), d);
  if (days === 0) return `今天 ${time}`;
  if (days === 1) return `明天 ${time}`;
  if (days === 2) return `后天 ${time}`;
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${time}`;
}

/**
 * Urgency changes with time: an unfinished task in a "not urgent" quadrant whose
 * deadline is near (or passed) belongs in the urgent row. Returns the suggested quadrant.
 */
export function urgentTarget(task: Task, now: number): Quadrant | null {
  if (task.done || task.due_at == null || dueState(task.due_at, now) === "later") return null;
  if (task.quadrant === 2) return 1;
  if (task.quadrant === 4) return 3;
  return null;
}

/** `<input type="datetime-local">` value in local time. */
export function toLocalInput(ms: number | null): string {
  if (ms == null) return "";
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function fromLocalInput(value: string): number | null {
  if (!value) return null;
  const ms = new Date(value).getTime(); // no zone suffix → parsed as local time
  return Number.isFinite(ms) ? ms : null;
}

export type QuickDue = "today" | "tomorrow" | "week" | "nextWeek" | "month" | "none";

/** One-tap deadlines, all at 23:59 local time; "week" ends on Sunday. */
export function quickDue(kind: QuickDue, now: number): number | null {
  if (kind === "none") return null;
  const d = new Date(now);
  const toSunday = (7 - d.getDay()) % 7;
  const days = { today: 0, tomorrow: 1, week: toSunday, nextWeek: toSunday + 7, month: 30 }[kind];
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + days, 23, 59).getTime();
}

/** Whole local calendar days from `a` to `b`; rounding absorbs 23/25-hour DST days. */
function calendarDaysBetween(a: Date, b: Date): number {
  const start = new Date(a.getFullYear(), a.getMonth(), a.getDate()).getTime();
  const end = new Date(b.getFullYear(), b.getMonth(), b.getDate()).getTime();
  return Math.round((end - start) / 86_400_000);
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}
