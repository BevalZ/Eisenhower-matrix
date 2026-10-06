import type { Quadrant, Task } from "./types";

export const PRIORITY_MIN = 0;
export const PRIORITY_MAX = 100;
/** Smallest gap kept between neighbours. Cards show rounded priority, so it stays invisible. */
export const PRIORITY_EPS = 0.001;
/** Gap used when a card is dropped above the first or below the last card. */
const EDGE_STEP = 5;

export interface TaskMove {
  id: number;
  quadrant: Quadrant;
  priority: number;
}

export interface ZoneRect {
  q: Quadrant;
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Board order: unfinished first, then higher priority; ties by id so the order never flickers. */
export function compareTasks(a: Task, b: Task): number {
  if (a.done !== b.done) return a.done ? 1 : -1;
  if (a.priority !== b.priority) return b.priority - a.priority;
  return a.id - b.id;
}

/**
 * Clamp a drop slot so unfinished and finished cards stay in their own groups.
 * `list` is the quadrant in board order without the dragged task.
 */
export function clampIndex(list: Task[], done: boolean, index: number): number {
  const undone = list.filter((t) => !t.done).length;
  const [min, max] = done ? [undone, list.length] : [0, undone];
  return Math.min(max, Math.max(min, index));
}

/** Slot where `task` would sort by its own priority (keyboard moves between quadrants). */
export function naturalIndex(list: Task[], task: Task): number {
  const i = list.findIndex((t) => compareTasks(task, t) < 0);
  return i === -1 ? list.length : i;
}

/** Drop slot for pointer `y`, given the vertical midpoints of the cards in DOM order. */
export function indexFromPointer(y: number, midpoints: number[]): number {
  let i = 0;
  while (i < midpoints.length && y > midpoints[i]) i++;
  return i;
}

/** How long a finger must rest on a card before it starts dragging instead of scrolling. */
export const TOUCH_HOLD_MS = 260;
/** Finger travel that turns a touch into a list scroll while the hold is still pending. */
export const TOUCH_SLOP = 8;

/**
 * What a touch that started on a card means right now. Cards disable native panning
 * (`touch-action: none`) so the list cannot scroll itself on a phone; the board scrolls it
 * by hand while `armed` is false, and a long enough hold (`armed`) turns the gesture into a
 * drag. Mouse pointers skip this and drag straight away.
 */
export function touchIntent(
  movedPx: number,
  armed: boolean,
  slop = TOUCH_SLOP,
): "drag" | "scroll" | "pending" {
  if (armed) return "drag";
  return movedPx > slop ? "scroll" : "pending";
}

/** Quadrant under the pointer; the small gaps between quadrants snap to the nearest one. */
export function pickQuadrant(x: number, y: number, zones: ZoneRect[], slack = 16): Quadrant | null {
  let best: Quadrant | null = null;
  let bestDist = slack;
  for (const z of zones) {
    const dx = Math.max(z.left - x, 0, x - z.right);
    const dy = Math.max(z.top - y, 0, y - z.bottom);
    const dist = Math.hypot(dx, dy);
    if (dist === 0) return z.q;
    if (dist <= bestDist) {
      best = z.q;
      bestDist = dist;
    }
  }
  return best;
}

/** Auto-scroll speed in px per frame while dragging near the top or bottom of a list. */
export function edgeScrollSpeed(y: number, top: number, bottom: number, zone = 40, max = 14): number {
  if (bottom - top < zone * 2) zone = (bottom - top) / 2;
  if (y < top + zone) return -max * Math.min(1, (top + zone - y) / zone);
  if (y > bottom - zone) return max * Math.min(1, (y - (bottom - zone)) / zone);
  return 0;
}

/**
 * Priority updates that put `task` at slot `index` of `quadrant`.
 * `list` is the target quadrant in board order, without `task`.
 * Only the dragged task changes, plus neighbours whose priorities are tied with the
 * drop position, so unrelated tasks keep their sync timestamps. [] means no change.
 */
export function planDrop(task: Task, quadrant: Quadrant, list: Task[], index: number): TaskMove[] {
  const at = clampIndex(list, task.done, index);
  const group = list.filter((t) => t.done === task.done);
  const k = at - (task.done ? list.length - group.length : 0);
  const next = placeValue(group.map((t) => t.priority), k, task.priority);
  const items = [...group.slice(0, k), task, ...group.slice(k)];

  const moves: TaskMove[] = [];
  items.forEach((t, i) => {
    const isDragged = t.id === task.id;
    if ((isDragged && t.quadrant !== quadrant) || Math.abs(next[i] - t.priority) > 1e-9) {
      moves.push({ id: t.id, quadrant: isDragged ? quadrant : t.quadrant, priority: next[i] });
    }
  });
  return moves;
}

const clamp = (p: number) => Math.min(PRIORITY_MAX, Math.max(PRIORITY_MIN, p));

function insert(values: number[], k: number, p: number): number[] {
  return [...values.slice(0, k), p, ...values.slice(k)];
}

/** Values of the group after inserting a card at `k`, strictly decreasing and in range. */
function placeValue(values: number[], k: number, own: number): number[] {
  const hi = k > 0 ? values[k - 1] : Infinity;
  const lo = k < values.length ? values[k] : -Infinity;
  const fits = (p: number) => p <= hi - PRIORITY_EPS && p >= lo + PRIORITY_EPS;

  const candidates = [own];
  if (Number.isFinite(hi) && Number.isFinite(lo)) candidates.push((hi + lo) / 2);
  else if (Number.isFinite(hi)) candidates.push(hi - EDGE_STEP);
  else if (Number.isFinite(lo)) candidates.push(lo + EDGE_STEP);
  for (const p of candidates.map(clamp)) {
    if (fits(p)) return insert(values, k, p);
  }

  // No room between the neighbours: sit next to one of them and push the tied run apart.
  for (const p of [hi - PRIORITY_EPS, lo + PRIORITY_EPS]) {
    const out = pushApart(insert(values, k, clamp(p)), k);
    if (out) return out;
  }

  // Everything is packed against a bound; spread the whole group evenly.
  const n = values.length + 1;
  return Array.from({ length: n }, (_, i) => PRIORITY_MAX - (i * (PRIORITY_MAX - PRIORITY_MIN)) / (n - 1));
}

function pushApart(out: number[], k: number): number[] | null {
  for (let j = k + 1; j < out.length && out[j] > out[j - 1] - PRIORITY_EPS; j++) {
    out[j] = out[j - 1] - PRIORITY_EPS;
  }
  for (let j = k - 1; j >= 0 && out[j] < out[j + 1] + PRIORITY_EPS; j--) {
    out[j] = out[j + 1] + PRIORITY_EPS;
  }
  return out.every((v) => v >= PRIORITY_MIN && v <= PRIORITY_MAX) ? out : null;
}
