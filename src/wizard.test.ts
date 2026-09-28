import { describe, expect, it } from "vitest";
import { quickDue, type QuickDue } from "./due";
import {
  addOption,
  emptyCustomOptions,
  estimateQuadrant,
  MAX_CUSTOM_OPTIONS,
  parseCustomOptions,
  removeOption,
  type Consequence,
  type Impact,
} from "./wizard";

// 2026-09-29 is a Tuesday, so "this week" ends in five days.
const now = new Date(2026, 8, 29, 10, 0).getTime();
const DAY = 86_400_000;

function estimate(impact: Impact, due: QuickDue, consequence: Consequence) {
  return estimateQuadrant({ impact, dueAt: quickDue(due, now), consequence }, now);
}

describe("offline quadrant estimate", () => {
  it("maps tapped answers to quadrants", () => {
    expect(estimate("core", "today", "severe")).toBe(1);
    expect(estimate("core", "month", "some")).toBe(2);
    expect(estimate("minor", "tomorrow", "some")).toBe(3);
    expect(estimate("minor", "none", "none")).toBe(4);
  });

  it("lets a severe consequence make a near deadline urgent", () => {
    expect(estimate("helpful", "week", "some")).toBe(2);
    expect(estimate("helpful", "week", "severe")).toBe(1);
  });

  it("scores typed deadlines by calendar days left", () => {
    const severe = (dueAt: number | null) =>
      estimateQuadrant({ impact: "minor", dueAt, consequence: "severe" }, now);
    expect(severe(now - 3 * DAY)).toBe(3); // overdue counts as due today
    expect(severe(now + 7 * DAY)).toBe(3);
    expect(severe(now + 8 * DAY)).toBe(4);
    expect(severe(null)).toBe(4);
  });
});

describe("custom wizard options", () => {
  it("appends a trimmed option without changing the original list", () => {
    const list = ["3天内"];
    expect(addOption(list, "  月底前 ")).toEqual(["3天内", "月底前"]);
    expect(list).toEqual(["3天内"]);
  });

  it("ignores blank, duplicate and built-in options", () => {
    const list = ["3天内"];
    expect(addOption(list, "   ")).toBe(list);
    expect(addOption(list, " 3天内")).toBe(list);
    expect(addOption(list, "今天", ["今天", "明天"])).toBe(list);
  });

  it("cuts an option to 100 characters", () => {
    expect(addOption([], "长".repeat(150))).toEqual(["长".repeat(100)]);
  });

  it("drops the oldest option when full", () => {
    const full = Array.from({ length: MAX_CUSTOM_OPTIONS }, (_, i) => `选项${i}`);
    const next = addOption(full, "新选项");
    expect(next).toHaveLength(MAX_CUSTOM_OPTIONS);
    expect(next[0]).toBe("选项1");
    expect(next[next.length - 1]).toBe("新选项");
  });

  it("removes an option by label", () => {
    expect(removeOption(["a", "b"], "a")).toEqual(["b"]);
    expect(removeOption(["a"], "c")).toEqual(["a"]);
  });

  it("starts empty when nothing valid is saved", () => {
    expect(parseCustomOptions(null)).toEqual(emptyCustomOptions());
    expect(parseCustomOptions("{oops")).toEqual(emptyCustomOptions());
    expect(parseCustomOptions("42")).toEqual(emptyCustomOptions());
    expect(parseCustomOptions("null")).toEqual(emptyCustomOptions());
  });

  it("keeps only strings that addOption would accept", () => {
    const saved = JSON.stringify({
      impact: ["影响客户续约", 3, null, "  ", " 影响客户续约 "],
      due: "3天内",
      consequence: ["说不清", "会被扣绩效"],
      unknown: ["x"],
    });
    expect(parseCustomOptions(saved)).toEqual({
      ...emptyCustomOptions(),
      impact: ["影响客户续约"],
      consequence: ["会被扣绩效"],
    });
  });

  it("keeps the newest saved options when there are too many", () => {
    const many = Array.from({ length: MAX_CUSTOM_OPTIONS + 3 }, (_, i) => `备注${i}`);
    expect(parseCustomOptions(JSON.stringify({ note: many })).note).toEqual(many.slice(3));
  });
});
