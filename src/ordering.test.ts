import { describe, expect, it } from "vitest";
import type { Quadrant, Task } from "./types";
import {
  PRIORITY_MAX,
  PRIORITY_MIN,
  clampIndex,
  compareTasks,
  edgeScrollSpeed,
  indexFromPointer,
  naturalIndex,
  pickQuadrant,
  planDrop,
  type TaskMove,
} from "./ordering";

function mk(id: number, priority: number, done = false, quadrant: Quadrant = 1): Task {
  return {
    id,
    title: `t${id}`,
    description: "",
    quadrant,
    priority,
    importance_score: 3,
    urgency_score: 3,
    done,
    created_at: 0,
    completed_at: done ? 1 : null,
    due_at: null,
  };
}

/** Apply moves and return the target quadrant in board order. */
function apply(list: Task[], task: Task, quadrant: Quadrant, moves: TaskMove[]): Task[] {
  const all = [...list, task].map((t) => ({ ...t }));
  for (const m of moves) {
    const t = all.find((x) => x.id === m.id)!;
    t.quadrant = m.quadrant;
    t.priority = m.priority;
  }
  return all.filter((t) => t.quadrant === quadrant).sort(compareTasks);
}

function expectLandsAt(list: Task[], task: Task, quadrant: Quadrant, index: number) {
  const moves = planDrop(task, quadrant, list, index);
  const result = apply(list, task, quadrant, moves);
  const at = clampIndex(list, task.done, index);
  expect(result.map((t) => t.id)).toEqual([
    ...list.slice(0, at).map((t) => t.id),
    task.id,
    ...list.slice(at).map((t) => t.id),
  ]);
  // Strict order around the dropped card, so clients without the id tie-break agree.
  const prev = result[at - 1];
  const next = result[at + 1];
  if (prev && prev.done === task.done) expect(prev.priority).toBeGreaterThan(result[at].priority);
  if (next && next.done === task.done) expect(result[at].priority).toBeGreaterThan(next.priority);
  for (const m of moves) {
    expect(m.priority).toBeGreaterThanOrEqual(PRIORITY_MIN);
    expect(m.priority).toBeLessThanOrEqual(PRIORITY_MAX);
  }
  return moves;
}

describe("compareTasks", () => {
  it("puts unfinished first, then higher priority, then lower id", () => {
    const sorted = [mk(3, 50, true), mk(2, 50), mk(1, 50), mk(4, 90)].sort(compareTasks);
    expect(sorted.map((t) => t.id)).toEqual([4, 1, 2, 3]);
  });
});

describe("clampIndex", () => {
  const list = [mk(1, 80), mk(2, 60), mk(3, 90, true)];
  it("keeps unfinished cards above finished ones", () => {
    expect(clampIndex(list, false, 3)).toBe(2);
    expect(clampIndex(list, false, 1)).toBe(1);
  });
  it("keeps finished cards below unfinished ones", () => {
    expect(clampIndex(list, true, 0)).toBe(2);
    expect(clampIndex(list, true, 3)).toBe(3);
  });
});

describe("naturalIndex", () => {
  it("finds the slot a task sorts into", () => {
    const list = [mk(1, 80), mk(2, 40), mk(3, 90, true)];
    expect(naturalIndex(list, mk(9, 60, false, 2))).toBe(1);
    expect(naturalIndex(list, mk(9, 10, false, 2))).toBe(2);
    expect(naturalIndex(list, mk(9, 10, true, 2))).toBe(3);
  });
});

describe("planDrop", () => {
  it("returns no moves when the card is dropped back where it was", () => {
    const list = [mk(1, 80), mk(2, 40)];
    expect(planDrop(mk(9, 50), 1, list, 1)).toEqual([]);
  });

  it("keeps the priority and only changes the quadrant when it already fits", () => {
    const list = [mk(1, 80), mk(2, 40)];
    const moves = expectLandsAt(list, mk(9, 50, false, 2), 1, 1);
    expect(moves).toEqual([{ id: 9, quadrant: 1, priority: 50 }]);
  });

  it("uses the midpoint between neighbours", () => {
    const moves = expectLandsAt([mk(1, 80), mk(2, 60), mk(3, 40)], mk(9, 30), 1, 1);
    expect(moves).toEqual([{ id: 9, quadrant: 1, priority: 70 }]);
  });

  it("steps above the first card and below the last card", () => {
    const list = [mk(1, 80), mk(2, 60)];
    expect(expectLandsAt(list, mk(9, 50), 1, 0)).toEqual([{ id: 9, quadrant: 1, priority: 85 }]);
    expect(expectLandsAt(list, mk(9, 90), 1, 2)).toEqual([{ id: 9, quadrant: 1, priority: 55 }]);
  });

  it("separates tied neighbours without touching the rest", () => {
    const list = [mk(1, 70), mk(2, 50), mk(3, 50), mk(4, 50), mk(5, 20)];
    const moves = expectLandsAt(list, mk(9, 10), 1, 2);
    const touched = moves.map((m) => m.id).sort();
    expect(touched).not.toContain(1);
    expect(touched).not.toContain(5);
  });

  it("handles cards packed at the top and bottom bounds", () => {
    expectLandsAt([mk(1, 100), mk(2, 100)], mk(9, 50), 1, 0);
    expectLandsAt([mk(1, 100), mk(2, 100)], mk(9, 50), 1, 1);
    expectLandsAt([mk(1, 0), mk(2, 0)], mk(9, 50), 1, 2);
  });

  it("keeps finished cards in the finished group", () => {
    const list = [mk(1, 80), mk(2, 60), mk(3, 90, true)];
    expectLandsAt(list, mk(9, 50, true), 1, 0);
    expectLandsAt(list, mk(9, 50), 1, 3);
  });

  it("always lands where it was dropped (random lists)", () => {
    let seed = 42;
    const rand = () => ((seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31);
    for (let round = 0; round < 500; round++) {
      const n = Math.floor(rand() * 8);
      const list = Array.from({ length: n }, (_, i) =>
        mk(i + 1, Math.round(rand() * 10) * 10, rand() < 0.3),
      ).sort(compareTasks);
      const task = mk(100, Math.round(rand() * 100), rand() < 0.3, 2);
      expectLandsAt(list, task, 1, Math.floor(rand() * (n + 1)));
    }
  });
});

describe("pointer helpers", () => {
  const zones = [
    { q: 2 as Quadrant, left: 0, top: 0, right: 100, bottom: 100 },
    { q: 1 as Quadrant, left: 112, top: 0, right: 212, bottom: 100 },
  ];

  it("picks the quadrant under the pointer and snaps across the gap", () => {
    expect(pickQuadrant(50, 50, zones)).toBe(2);
    expect(pickQuadrant(150, 50, zones)).toBe(1);
    expect(pickQuadrant(104, 50, zones)).toBe(2);
    expect(pickQuadrant(109, 50, zones)).toBe(1);
    expect(pickQuadrant(500, 50, zones)).toBeNull();
  });

  it("maps the pointer to a slot using card midpoints", () => {
    expect(indexFromPointer(5, [20, 60, 100])).toBe(0);
    expect(indexFromPointer(61, [20, 60, 100])).toBe(2);
    expect(indexFromPointer(500, [20, 60, 100])).toBe(3);
    expect(indexFromPointer(5, [])).toBe(0);
  });

  it("scrolls faster closer to the edge", () => {
    expect(edgeScrollSpeed(200, 0, 400)).toBe(0);
    expect(edgeScrollSpeed(30, 0, 400)).toBeLessThan(0);
    expect(edgeScrollSpeed(0, 0, 400)).toBe(-14);
    expect(edgeScrollSpeed(399, 0, 400)).toBeGreaterThan(edgeScrollSpeed(370, 0, 400));
  });
});
