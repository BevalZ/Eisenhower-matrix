import { describe, expect, it } from "vitest";
import {
  CARD_GAP, LIST_PADDING, rowHeights, rowOffsets, rowWindow, slotIndexAt,
} from "./virtualRows";
import type { Task } from "./types";

function task(id: number, description = ""): Task {
  return {
    id, title: `task ${id}`, description, quadrant: 1, priority: 50,
    importance_score: 3, urgency_score: 3, done: false, created_at: 1,
    completed_at: null, due_at: null,
  };
}

/** Rows of 68px with the 8px card gap, starting at the list's 10px padding edge. */
const uniform = (count: number, height = 76) => rowOffsets(new Array(count).fill(height));

describe("quadrant windowing", () => {
  it("mounts only the rows around the viewport and pads the rest", () => {
    const offsets = uniform(1000);
    const top = rowWindow(offsets, 0, 500, 2);
    expect(top.start).toBe(0);
    // 500px of viewport over 76px rows, one partial row, plus two rows of overscan.
    expect(top.end).toBe(9);
    expect(top.topPad).toBe(0);
    expect(top.bottomPad).toBeCloseTo(offsets[1000] - offsets[top.end]);

    const middle = rowWindow(offsets, 7_600, 500, 2);
    expect(middle.start).toBe(97);
    expect(middle.end).toBe(109);
    // Spacers, the list's own padding and the mounted rows add up to the whole list height.
    expect(middle.total).toBeCloseTo(offsets[1000]);
    expect(middle.topPad + LIST_PADDING + (offsets[middle.end] - offsets[middle.start]) + middle.bottomPad)
      .toBeCloseTo(middle.total);
  });

  it("keeps the window inside the list at either end", () => {
    const offsets = uniform(50);
    expect(rowWindow(offsets, 0, 400)).toMatchObject({ start: 0 });
    const bottom = rowWindow(offsets, 10_000, 400);
    expect(bottom.end).toBe(50);
    expect(bottom.bottomPad).toBe(0);
    expect(bottom.topPad).toBeGreaterThan(0);
    expect(rowWindow(rowOffsets([]), 0, 400))
      .toEqual({ start: 0, end: 0, topPad: 0, bottomPad: 0, total: LIST_PADDING });
  });

  it("estimates rows it has not measured yet", () => {
    const rows = [task(1), task(2, "描述"), task(3)];
    const measured = new Map([[1, 70]]);
    expect(rowHeights(rows, measured, { plain: 80, described: 120 })).toEqual([70, 120, 80]);
  });

  it("maps a pointer to the slot between the rows it falls between", () => {
    const offsets = uniform(4);
    const at = (y: number) => slotIndexAt(offsets, y);
    expect(at(0)).toBe(0);
    // Slot boundaries sit on the row midpoints: 48, 124, 200, 276.
    expect(at(48)).toBe(0);
    expect(at(49)).toBe(1);
    expect(at(200)).toBe(2);
    expect(at(10_000)).toBe(4);
    expect(at(-50)).toBe(0);
  });

  it("counts the placeholder gap so the slot does not flicker", () => {
    const offsets = uniform(4);
    const placeholder = { index: 1, height: 76 + CARD_GAP };
    const at = (y: number) => slotIndexAt(offsets, y, placeholder);
    // Anywhere inside the gap the slot stays where the placeholder already is.
    expect(at(100)).toBe(1);
    expect(at(160)).toBe(1);
    // Below the gap the rows sit one card lower, so the same pointer means a later slot.
    expect(at(215)).toBe(2);
    expect(at(220)).toBe(2);
    expect(at(10_000)).toBe(4);
  });
});
