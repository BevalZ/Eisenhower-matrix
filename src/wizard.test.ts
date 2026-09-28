import { describe, expect, it } from "vitest";
import { estimateQuadrant } from "./wizard";

describe("offline quadrant estimate", () => {
  it("maps tapped answers to quadrants", () => {
    expect(estimateQuadrant({ impact: "core", due: "today", consequence: "severe" })).toBe(1);
    expect(estimateQuadrant({ impact: "core", due: "month", consequence: "some" })).toBe(2);
    expect(estimateQuadrant({ impact: "minor", due: "tomorrow", consequence: "some" })).toBe(3);
    expect(estimateQuadrant({ impact: "minor", due: "none", consequence: "none" })).toBe(4);
  });

  it("lets a severe consequence make a near deadline urgent", () => {
    expect(estimateQuadrant({ impact: "helpful", due: "week", consequence: "some" })).toBe(2);
    expect(estimateQuadrant({ impact: "helpful", due: "week", consequence: "severe" })).toBe(1);
  });
});
