import type { Task } from "./types";
import { lowerBound } from "./ordering";

/**
 * Windowing turns a quadrant list into a fixed number of mounted cards plus two spacers that
 * stand in for everything above and below the viewport. It only kicks in past
 * `WINDOW_FROM` rows: below that, every card stays mounted, which keeps small boards on the
 * plain flow layout (and keeps the drop-slot geometry exact) while a board with thousands of
 * active tasks stops paying for thousands of DOM subtrees.
 */
export const WINDOW_FROM = 200;
/** Extra rows kept mounted beyond the viewport so a fast scroll never shows a blank strip. */
export const OVERSCAN = 6;
/** `.q-body` padding-top: card offsets are measured from the padding edge, so the first row starts here. */
export const LIST_PADDING = 10;
/** `.task-card` margin-bottom; `offsetHeight` excludes margins, so measuring has to add it back. */
export const CARD_GAP = 8;
/** Card heights used until the rows on screen have been measured. */
export const DEFAULT_HEIGHT = 78;
export const DEFAULT_HEIGHT_WITH_DESCRIPTION = 100;

/** Rolling averages of what cards actually measure; stand-in heights for rows never seen. */
export interface HeightAverages {
  plain: number;
  described: number;
}

/** Measured height where we have one, otherwise the average for that shape of card. */
export function rowHeights(
  rows: Task[],
  measured: Map<number, number>,
  averages: HeightAverages,
): number[] {
  const out = new Array<number>(rows.length);
  for (let i = 0; i < rows.length; i++) {
    const task = rows[i];
    out[i] = measured.get(task.id) ?? (task.description ? averages.described : averages.plain);
  }
  return out;
}

/** Offset of each row from the padding edge; `offsets[n]` is the total list height. */
export function rowOffsets(heights: number[]): Float64Array {
  const offsets = new Float64Array(heights.length + 1);
  let y = LIST_PADDING;
  for (let i = 0; i < heights.length; i++) {
    offsets[i] = y;
    y += heights[i];
  }
  offsets[heights.length] = y;
  return offsets;
}

export interface RowWindow {
  /** First mounted row. */
  start: number;
  /** One past the last mounted row. */
  end: number;
  /** Height of the spacer above the mounted rows. */
  topPad: number;
  /** Height of the spacer below them. */
  bottomPad: number;
  /** Total list height, spacers included. */
  total: number;
}

export function rowWindow(
  offsets: Float64Array,
  scrollTop: number,
  viewport: number,
  overscan = OVERSCAN,
): RowWindow {
  const count = offsets.length - 1;
  if (count === 0) return { start: 0, end: 0, topPad: 0, bottomPad: 0, total: offsets[0] };
  const first = firstRowAt(offsets, scrollTop);
  const last = firstRowAt(offsets, scrollTop + Math.max(viewport, 0));
  const start = Math.max(0, first - overscan);
  const end = Math.min(count, last + 1 + overscan);
  return {
    start,
    end,
    topPad: offsets[start] - LIST_PADDING,
    bottomPad: Math.max(0, offsets[count] - offsets[end]),
    total: offsets[count],
  };
}

/** Index of the first row whose bottom edge is below `y`. */
function firstRowAt(offsets: Float64Array, y: number): number {
  return lowerBound(offsets.length - 1, (i) => offsets[i + 1] > y);
}

/**
 * Drop slot for a pointer at `y`: the number of row midpoints above it.
 *
 * `placeholder` is the gap the dragged card currently occupies. Rows below it sit that much
 * lower on screen, and counting them at their real position is what keeps the slot from
 * flickering by one while the pointer rests inside the gap.
 */
export function slotIndexAt(
  offsets: Float64Array,
  y: number,
  placeholder?: { index: number; height: number },
): number {
  const count = offsets.length - 1;
  const shift = placeholder ? placeholder.height : 0;
  const from = placeholder ? placeholder.index : count;
  return lowerBound(count, (i) => y <= offsets[i] + (offsets[i + 1] - offsets[i]) / 2 + (i >= from ? shift : 0));
}
