import { describe, expect, it } from "vitest";
import type { Quadrant, Task } from "./types";
import { dueInDays, dueLabel, dueState, fromLocalInput, parseDueText, quickDue, toLocalInput, urgentTarget } from "./due";

const at = (y: number, m: number, d: number, h = 0, min = 0) => new Date(y, m - 1, d, h, min).getTime();

function task(quadrant: Quadrant, due_at: number | null, done = false): Task {
  return {
    id: 1, title: "t", description: "", quadrant, priority: 50,
    importance_score: 3, urgency_score: 1, done, created_at: 0, completed_at: null, due_at,
  };
}

describe("due dates", () => {
  const now = at(2026, 9, 27, 10, 0);

  it("classifies overdue, soon and later", () => {
    expect(dueState(now - 1, now)).toBe("overdue");
    expect(dueState(now + 47 * 3_600_000, now)).toBe("soon");
    expect(dueState(now + 49 * 3_600_000, now)).toBe("later");
  });

  it("labels by local calendar day", () => {
    expect(dueLabel(now - 60_000, now)).toBe("已逾期");
    expect(dueLabel(at(2026, 9, 27, 18, 0), now)).toBe("今天 18:00");
    expect(dueLabel(at(2026, 9, 28, 9, 30), now)).toBe("明天 09:30");
    expect(dueLabel(at(2026, 9, 29, 8, 0), now)).toBe("后天 08:00");
    expect(dueLabel(at(2026, 10, 3, 12, 0), now)).toBe("10-03 12:00");
  });

  it("suggests the urgent row only for near deadlines in non-urgent quadrants", () => {
    const soon = now + 3_600_000;
    expect(urgentTarget(task(2, soon), now)).toBe(1);
    expect(urgentTarget(task(4, now - 1), now)).toBe(3);
    expect(urgentTarget(task(1, soon), now)).toBeNull();
    expect(urgentTarget(task(2, soon, true), now)).toBeNull();
    expect(urgentTarget(task(2, now + 100 * 3_600_000), now)).toBeNull();
    expect(urgentTarget(task(2, null), now)).toBeNull();
  });

  it("round-trips datetime-local values in local time", () => {
    const ms = at(2026, 12, 31, 23, 45);
    expect(toLocalInput(ms)).toBe("2026-12-31T23:45");
    expect(fromLocalInput("2026-12-31T23:45")).toBe(ms);
    expect(fromLocalInput("")).toBeNull();
    expect(toLocalInput(null)).toBe("");
  });

  it("maps one-tap deadlines to 23:59 local time", () => {
    // 2026-09-27 is a Sunday.
    expect(quickDue("today", now)).toBe(at(2026, 9, 27, 23, 59));
    expect(quickDue("tomorrow", now)).toBe(at(2026, 9, 28, 23, 59));
    expect(quickDue("week", now)).toBe(at(2026, 9, 27, 23, 59));
    expect(quickDue("nextWeek", now)).toBe(at(2026, 10, 4, 23, 59));
    expect(quickDue("week", at(2026, 9, 29, 8))).toBe(at(2026, 10, 4, 23, 59));
    expect(quickDue("month", now)).toBe(at(2026, 10, 27, 23, 59));
    expect(quickDue("none", now)).toBeNull();
  });

  it("counts calendar days to 23:59", () => {
    expect(dueInDays(0, now)).toBe(at(2026, 9, 27, 23, 59));
    expect(dueInDays(5, now)).toBe(at(2026, 10, 2, 23, 59));
  });
});

describe("typed deadlines", () => {
  // 2026-09-27 10:00 is a Sunday.
  const now = at(2026, 9, 27, 10, 0);
  const wednesday = at(2026, 9, 30, 10, 0);
  const due = (text: string, from = now) => parseDueText(text, from);
  const end = (y: number, m: number, d: number) => at(y, m, d, 23, 59);

  it("reads days, weeks and months from now", () => {
    expect(due("今天")).toBe(end(2026, 9, 27));
    expect(due("明天前")).toBe(end(2026, 9, 28));
    expect(due("大后天")).toBe(end(2026, 9, 30));
    expect(due("3天内")).toBe(end(2026, 9, 30));
    expect(due("三天之内")).toBe(end(2026, 9, 30));
    expect(due(" 3 天 ")).toBe(end(2026, 9, 30));
    expect(due("十二天")).toBe(end(2026, 10, 9));
    expect(due("两周")).toBe(end(2026, 10, 11));
    expect(due("一个星期以内")).toBe(end(2026, 10, 4));
    expect(due("一个月")).toBe(end(2026, 10, 27));
  });

  it("keeps a month count inside a short month", () => {
    expect(due("两个月", at(2026, 12, 31, 10))).toBe(end(2027, 2, 28));
    expect(due("1个月", at(2028, 1, 31, 10))).toBe(end(2028, 2, 29));
  });

  it("reads weekdays with weeks starting on Monday", () => {
    expect(due("周五前")).toBe(end(2026, 10, 2));
    expect(due("星期一")).toBe(end(2026, 9, 28));
    expect(due("周日")).toBe(end(2026, 9, 27));
    expect(due("周末")).toBe(end(2026, 9, 27));
    expect(due("下周三")).toBe(end(2026, 9, 30));
    expect(due("下周日")).toBe(end(2026, 10, 4));
    expect(due("下下周一")).toBe(end(2026, 10, 5));
    expect(due("下周内")).toBe(end(2026, 10, 4));
    expect(due("周三", wednesday)).toBe(end(2026, 9, 30));
    expect(due("周二", wednesday)).toBe(end(2026, 10, 6));
    expect(due("下周三", wednesday)).toBe(end(2026, 10, 7));
    expect(due("下周", wednesday)).toBe(end(2026, 10, 11));
  });

  it("reads month ends and dates, rolling a passed date to next year", () => {
    expect(due("月底前")).toBe(end(2026, 9, 30));
    expect(due("本月内")).toBe(end(2026, 9, 30));
    expect(due("10月5日")).toBe(end(2026, 10, 5));
    expect(due("10月5号前")).toBe(end(2026, 10, 5));
    expect(due("10/5")).toBe(end(2026, 10, 5));
    expect(due("9月27日")).toBe(end(2026, 9, 27));
    expect(due("9月26日")).toBe(end(2027, 9, 26));
    expect(due("2027年1月3日")).toBe(end(2027, 1, 3));
    expect(due("2027-1-3")).toBe(end(2027, 1, 3));
  });

  it("returns null for no deadline and undefined when not understood", () => {
    expect(due("没有")).toBeNull();
    expect(due("不急")).toBeNull();
    expect(due("2月30日")).toBeUndefined();
    expect(due("13月1日")).toBeUndefined();
    expect(due("10月0日")).toBeUndefined();
    expect(due("0天")).toBeUndefined();
    expect(due("随便什么时候")).toBeUndefined();
  });
});
