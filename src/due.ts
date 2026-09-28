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
  const toSunday = (7 - new Date(now).getDay()) % 7;
  const days = { today: 0, tomorrow: 1, week: toSunday, nextWeek: toSunday + 7, month: 30 }[kind];
  return dueInDays(days, now);
}

/** 23:59 local time, `days` calendar days from now (0 = today). */
export function dueInDays(days: number, now: number): number {
  const d = new Date(now);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + days, 23, 59).getTime();
}

const CN_NUM: Record<string, number> = { 一: 1, 二: 2, 两: 2, 三: 3, 四: 4, 五: 5, 六: 6, 七: 7, 八: 8, 九: 9 };
const WEEKDAY: Record<string, number> = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0, 七: 0 };

/** "3", "十", "十二", "二十" → number; anything else → NaN. */
function parseCount(s: string): number {
  if (/^\d+$/.test(s)) return Number(s);
  const m = /^([一二两三四五六七八九])?(十)?([一二三四五六七八九])?$/.exec(s);
  if (!m || (!m[2] && m[3])) return NaN;
  if (!m[2]) return m[1] ? CN_NUM[m[1]] : NaN;
  return (m[1] ? CN_NUM[m[1]] : 1) * 10 + (m[3] ? CN_NUM[m[3]] : 0);
}

/**
 * A typed deadline such as "3天内", "两周", "周五前", "下周三", "月底", "10月5日".
 * Returns the due time (23:59 local), null for "no deadline", undefined if not understood.
 */
export function parseDueText(text: string, now: number): number | null | undefined {
  const s = text.replace(/\s+/g, "").replace(/(之前|以前|之内|以内|前|内|后|左右)$/, "");
  if (/^(没有|无|不限|没有截止|无截止|不急)$/.test(s)) return null;
  const d = new Date(now);
  const relative: Record<string, number> = { 今天: 0, 今晚: 0, 明天: 1, 后天: 2, 大后天: 3 };
  if (s in relative) return dueInDays(relative[s], now);

  let m = /^(\d+|[一二两三四五六七八九十]+)个?(天|日|周|星期|礼拜|月)$/.exec(s);
  if (m) {
    const n = parseCount(m[1]);
    if (!(n > 0 && n <= 3650)) return undefined;
    if (m[2] !== "月") return dueInDays(m[2] === "天" || m[2] === "日" ? n : n * 7, now);
    const lastDay = new Date(d.getFullYear(), d.getMonth() + n + 1, 0).getDate();
    return new Date(d.getFullYear(), d.getMonth() + n, Math.min(d.getDate(), lastDay), 23, 59).getTime();
  }

  const today = d.getDay();
  if (s === "周末" || s === "本周" || s === "这周") return quickDue("week", now);
  if (s === "下周") return quickDue("nextWeek", now);
  if (s === "月底" || s === "本月") {
    return new Date(d.getFullYear(), d.getMonth() + 1, 0, 23, 59).getTime();
  }
  m = /^(下下|下|本|这)?(周|星期|礼拜)([一二三四五六日天七])$/.exec(s);
  if (m) {
    const wd = WEEKDAY[m[3]];
    if (!m[1] || m[1] === "本" || m[1] === "这") return dueInDays((wd - today + 7) % 7, now);
    // Weeks start on Monday: find next Monday, then the weekday within that week.
    const toNextMonday = (1 - today + 7) % 7 || 7;
    const extra = m[1] === "下下" ? 7 : 0;
    return dueInDays(toNextMonday + extra + (wd === 0 ? 6 : wd - 1), now);
  }

  m = /^(?:(\d{4})[年\-/.])?(\d{1,2})[月\-/.](\d{1,2})[日号]?$/.exec(s);
  if (m) {
    const month = Number(m[2]) - 1;
    const day = Number(m[3]);
    let year = m[1] ? Number(m[1]) : d.getFullYear();
    const at = (y: number) => new Date(y, month, day, 23, 59);
    let due = at(year);
    if (!m[1] && due.getTime() < now) due = at(++year);
    if (due.getMonth() !== month || due.getDate() !== day) return undefined;
    return due.getTime();
  }
  return undefined;
}

/** Whole local calendar days from `a` to `b`; rounding absorbs 23/25-hour DST days. */
export function calendarDaysBetween(a: Date, b: Date): number {
  const start = new Date(a.getFullYear(), a.getMonth(), a.getDate()).getTime();
  const end = new Date(b.getFullYear(), b.getMonth(), b.getDate()).getTime();
  return Math.round((end - start) / 86_400_000);
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}
