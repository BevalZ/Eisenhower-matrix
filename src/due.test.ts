import { describe, expect, it } from "vitest";
import type { Quadrant, Task } from "./types";
import { dueLabel, dueState, fromLocalInput, quickDue, toLocalInput, urgentTarget } from "./due";

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
});
