import { nextTick, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import type { Quadrant, Task } from "../types";
import {
  clampIndex,
  edgeScrollSpeed,
  indexFromPointer,
  pickQuadrant,
  planDrop,
  type TaskMove,
  type ZoneRect,
} from "../ordering";

const MOUSE_THRESHOLD = 4;
const TOUCH_THRESHOLD = 8;
const SETTLE_MS = 180;

export interface DropSlot {
  q: Quadrant;
  index: number;
}

interface Options {
  /** Quadrant in board order, including the dragged task. */
  list: (q: Quadrant) => Task[];
  /** Apply moves (optimistic part must run synchronously) and resolve once saved. */
  commit: (moves: TaskMove[]) => Promise<void>;
  /** Called when a drag starts; the returned function is called after the drop is saved. */
  lock: () => () => void;
}

/**
 * Pointer-events drag and drop for the quadrant board.
 * The dragged card is taken out of its list, a placeholder marks the drop slot,
 * and a ghost copy follows the pointer and settles into the slot on release.
 */
export function useBoardDrag(opts: Options) {
  const dragging = shallowRef<Task | null>(null);
  const slot = ref<DropSlot | null>(null);
  const outside = ref(false);
  const settling = ref(false);
  const ghostSize = ref({ width: 0, height: 0 });
  const ghostEl = ref<HTMLElement | null>(null);

  const zones = new Map<Quadrant, HTMLElement>();
  const bodies = new Map<Quadrant, HTMLElement>();

  let pointerId = -1;
  let pointerType = "mouse";
  let startX = 0;
  let startY = 0;
  let x = 0;
  let y = 0;
  let offsetX = 0;
  let offsetY = 0;
  let pending: { task: Task; card: HTMLElement } | null = null;
  let origin: DropSlot | null = null;
  let raf = 0;
  let unlock: (() => void) | null = null;

  function setZone(q: Quadrant, el: unknown) {
    if (el instanceof HTMLElement) zones.set(q, el);
    else zones.delete(q);
  }

  function setBody(q: Quadrant, el: unknown) {
    if (el instanceof HTMLElement) bodies.set(q, el);
    else bodies.delete(q);
  }

  function onPointerDown(e: PointerEvent, task: Task) {
    if (dragging.value || settling.value || pending) return;
    if (!e.isPrimary || e.button !== 0) return;
    const card = (e.target as Element | null)?.closest<HTMLElement>("[data-task-id]");
    if (!card) return;
    pending = { task, card };
    pointerId = e.pointerId;
    pointerType = e.pointerType;
    startX = x = e.clientX;
    startY = y = e.clientY;
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerCancel);
  }

  function onPointerMove(e: PointerEvent) {
    if (e.pointerId !== pointerId) return;
    x = e.clientX;
    y = e.clientY;
    if (!dragging.value) {
      const threshold = pointerType === "mouse" ? MOUSE_THRESHOLD : TOUCH_THRESHOLD;
      if (Math.hypot(x - startX, y - startY) < threshold) return;
      start();
    }
    e.preventDefault();
    schedule();
  }

  function start() {
    if (!pending) return;
    const { task, card } = pending;
    const rect = card.getBoundingClientRect();
    offsetX = startX - rect.left;
    offsetY = startY - rect.top;
    ghostSize.value = { width: rect.width, height: rect.height };
    origin = { q: task.quadrant, index: opts.list(task.quadrant).findIndex((t) => t.id === task.id) };
    slot.value = { ...origin };
    outside.value = false;
    dragging.value = task;
    unlock = opts.lock();
    window.getSelection()?.removeAllRanges();
    document.documentElement.classList.add("board-dragging");
    // Keep receiving pointer events even when the pointer leaves the window.
    try {
      document.documentElement.setPointerCapture(pointerId);
    } catch {
      /* the pointer may already be gone */
    }
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("blur", onBlur);
  }

  function schedule() {
    if (!raf) raf = requestAnimationFrame(frame);
  }

  function frame() {
    raf = 0;
    if (!dragging.value || settling.value) return;
    placeGhost();
    const q = track();
    if (q !== null && autoScroll(q)) schedule();
  }

  function placeGhost() {
    const el = ghostEl.value;
    if (el) el.style.transform = `translate3d(${x - offsetX}px, ${y - offsetY}px, 0) rotate(1.5deg) scale(1.03)`;
  }

  /** Update the drop slot from the pointer; returns the quadrant under it. */
  function track(): Quadrant | null {
    const task = dragging.value;
    if (!task) return null;
    const q = pickQuadrant(x, y, zoneRects());
    outside.value = q === null;
    setSlot(q === null ? origin : { q, index: slotIndex(q, task) });
    return q;
  }

  function zoneRects(): ZoneRect[] {
    return [...zones].map(([q, el]) => {
      const r = el.getBoundingClientRect();
      return { q, left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    });
  }

  function slotIndex(q: Quadrant, task: Task): number {
    const list = opts.list(q).filter((t) => t.id !== task.id);
    const body = bodies.get(q);
    if (!body) return clampIndex(list, task.done, list.length);
    // offsetTop ignores the FLIP transforms of cards that are still animating.
    const top = body.getBoundingClientRect().top + body.clientTop - body.scrollTop;
    const cards = body.querySelectorAll<HTMLElement>("[data-task-id]");
    const mids = Array.from(cards, (c) => top + c.offsetTop + c.offsetHeight / 2);
    return clampIndex(list, task.done, indexFromPointer(y, mids));
  }

  function autoScroll(q: Quadrant): boolean {
    const body = bodies.get(q);
    if (!body) return false;
    const r = body.getBoundingClientRect();
    const speed = edgeScrollSpeed(y, r.top, r.bottom);
    if (!speed) return false;
    const before = body.scrollTop;
    body.scrollTop += speed;
    return body.scrollTop !== before;
  }

  function setSlot(next: DropSlot | null) {
    const cur = slot.value;
    if (next && (!cur || cur.q !== next.q || cur.index !== next.index)) slot.value = { ...next };
  }

  async function onPointerUp(e: PointerEvent) {
    if (e.pointerId !== pointerId) return;
    const task = dragging.value;
    if (!task) {
      reset();
      return;
    }
    x = e.clientX;
    y = e.clientY;
    track();
    const target = slot.value;
    const moves =
      target && !sameSlot(target, origin)
        ? planDrop(task, target.q, opts.list(target.q).filter((t) => t.id !== task.id), target.index)
        : [];
    if (!moves.length) {
      await cancel();
      return;
    }
    removePointerListeners();
    settling.value = true;
    await settleGhost();
    finish(opts.commit(moves));
  }

  /** Fly the ghost back to where the card came from. */
  async function cancel() {
    if (!dragging.value || settling.value) return;
    removePointerListeners();
    setSlot(origin);
    outside.value = false;
    settling.value = true;
    await settleGhost();
    finish(Promise.resolve());
  }

  /** Animate the ghost onto the placeholder of the current slot. */
  async function settleGhost() {
    await nextTick();
    const el = ghostEl.value;
    const q = slot.value?.q;
    const target = q ? bodies.get(q)?.querySelector<HTMLElement>(".drop-slot") : null;
    if (!el || !target || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    target.scrollIntoView({ block: "nearest" });
    const r = target.getBoundingClientRect();
    await new Promise<void>((resolve) => {
      const done = () => {
        clearTimeout(timer);
        el.removeEventListener("transitionend", done);
        resolve();
      };
      const timer = setTimeout(done, SETTLE_MS + 80);
      el.addEventListener("transitionend", done);
      el.style.transform = `translate3d(${r.left}px, ${r.top}px, 0)`;
    });
  }

  function finish(saved: Promise<void>) {
    cancelAnimationFrame(raf);
    raf = 0;
    removePointerListeners();
    window.removeEventListener("keydown", onKeyDown, true);
    window.removeEventListener("blur", onBlur);
    document.documentElement.classList.remove("board-dragging");
    try {
      if (pointerId >= 0) document.documentElement.releasePointerCapture(pointerId);
    } catch {
      /* already released */
    }
    dragging.value = null;
    slot.value = null;
    origin = null;
    pending = null;
    pointerId = -1;
    settling.value = false;
    outside.value = false;
    // Remote refreshes resume only after the drop is saved, so they cannot undo it.
    const release = unlock;
    unlock = null;
    if (release) saved.then(release, release);
  }

  function reset() {
    removePointerListeners();
    pending = null;
    pointerId = -1;
  }

  function removePointerListeners() {
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", onPointerCancel);
  }

  function onPointerCancel(e: PointerEvent) {
    if (e.pointerId !== pointerId) return;
    if (dragging.value) void cancel();
    else reset();
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key !== "Escape" || !dragging.value) return;
    e.preventDefault();
    e.stopPropagation();
    void cancel();
  }

  function onBlur() {
    if (dragging.value) void cancel();
  }

  // Position the ghost as soon as it is mounted, before the first paint.
  watch(ghostEl, (el) => {
    if (el) placeGhost();
  }, { flush: "post" });

  onBeforeUnmount(() => {
    if (dragging.value) finish(Promise.resolve());
    else reset();
  });

  return { dragging, slot, outside, settling, ghostSize, ghostEl, setZone, setBody, onPointerDown };
}

function sameSlot(a: DropSlot | null, b: DropSlot | null): boolean {
  return !!a && !!b && a.q === b.q && a.index === b.index;
}
