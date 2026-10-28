<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, onUpdated, ref } from "vue";
import TaskCard from "./TaskCard.vue";
import { QUADRANT_META, type Quadrant, type Task } from "../types";
import {
  CARD_GAP, DEFAULT_HEIGHT, DEFAULT_HEIGHT_WITH_DESCRIPTION, LIST_PADDING, WINDOW_FROM,
  rowHeights, rowOffsets, rowWindow, slotIndexAt, type HeightAverages,
} from "../virtualRows";

/** What the board needs from a column to run drag and keyboard moves. */
export interface ColumnHandle {
  body: HTMLElement;
  /** Drop slot for a pointer at this viewport y, or null while every card is mounted. */
  indexAtPointer: (clientY: number) => number | null;
  /** Scroll a card into view before focusing it (windowed columns only). */
  ensureVisible: (id: number) => Promise<void>;
}

const props = defineProps<{
  quadrant: Quadrant;
  /** Board order, without the card being dragged. */
  rows: Task[];
  /** Index of the drop placeholder inside this column, or null. */
  slotIndex: number | null;
  ghostHeight: number;
  /** Minute clock for the deadline badges; undefined when nothing here has a deadline. */
  now?: number;
}>();

const emit = defineEmits<{
  toggle: [id: number];
  remove: [id: number];
  "pointer-down": [e: PointerEvent, task: Task];
  "move-to": [task: Task, quadrant: Quadrant];
  nudge: [task: Task, delta: -1 | 1];
  edit: [task: Task];
  "body-click": [e: MouseEvent];
  ready: [handle: ColumnHandle | null];
}>();

const body = ref<HTMLElement | null>(null);
// A column only pays for windowing past WINDOW_FROM cards; smaller ones render in full, where
// the drop slot is measured from the real DOM and nothing about the layout changes.
const windowed = computed(() => props.rows.length > WINDOW_FROM);

const heights = new Map<number, number>();
/** Bumped whenever a measured height changed, so the offsets recompute. */
const version = ref(0);
const scrollTop = ref(0);
const viewport = ref(0);
const scrolled = ref(false);
let averages: HeightAverages = {
  plain: DEFAULT_HEIGHT,
  described: DEFAULT_HEIGHT_WITH_DESCRIPTION,
};

const offsets = computed(() => {
  version.value;
  return rowOffsets(rowHeights(props.rows, heights, averages));
});

const window = computed(() => (windowed.value
  ? rowWindow(offsets.value, scrollTop.value, viewport.value)
  : { start: 0, end: props.rows.length, topPad: 0, bottomPad: 0 }));

const windowStart = computed(() => window.value.start);
const windowEnd = computed(() => window.value.end);
const topPad = computed(() => window.value.topPad);
const bottomPad = computed(() => window.value.bottomPad);
/** Rows arriving because the list scrolled must not replay the entrance animation. */
const enter = computed(() => !windowed.value || !scrolled.value);

type Row = { key: string; slot: true } | { key: string; slot: false; task: Task };

/** The mounted slice of the list, with the drop placeholder at its current slot. */
const items = computed<Row[]>(() => {
  const start = windowStart.value;
  const end = windowEnd.value;
  const slot = props.slotIndex;
  const out: Row[] = [];
  for (let i = start; i < end; i++) {
    if (slot === i) out.push({ key: "drop-slot", slot: true });
    const task = props.rows[i];
    out.push({ key: `t${task.id}`, slot: false, task });
  }
  if (slot === end) out.push({ key: "drop-slot", slot: true });
  return out;
});

let scrollFrame = 0;
function onScroll() {
  if (scrollFrame) return;
  scrollFrame = requestAnimationFrame(() => {
    scrollFrame = 0;
    scrollTop.value = body.value?.scrollTop ?? 0;
    if (scrollTop.value > 0) scrolled.value = true;
  });
}

let measureFrame = 0;
function scheduleMeasure() {
  if (measureFrame) return;
  measureFrame = requestAnimationFrame(() => {
    measureFrame = 0;
    measure();
  });
}

/** Read back the height of every mounted card; the two spacers stand in for the rest. */
function measure() {
  const el = body.value;
  if (!el) return;
  let plain = 0;
  let plainCount = 0;
  let described = 0;
  let describedCount = 0;
  let changed = false;
  for (const node of el.querySelectorAll<HTMLElement>("[data-task-id]")) {
    const height = node.offsetHeight + CARD_GAP;
    const id = Number(node.dataset.taskId);
    if (heights.get(id) !== height) {
      heights.set(id, height);
      changed = true;
    }
    if (node.querySelector(".desc")) {
      described += height;
      describedCount++;
    } else {
      plain += height;
      plainCount++;
    }
  }
  // Cards that left the board should not keep their measurement forever.
  if (heights.size > props.rows.length * 2 + 32) heights.clear();
  if (plainCount) {
    const next = plain / plainCount;
    if (Math.abs(next - averages.plain) > 0.5) {
      averages = { ...averages, plain: next };
      changed = true;
    }
  }
  if (describedCount) {
    const next = described / describedCount;
    if (Math.abs(next - averages.described) > 0.5) {
      averages = { ...averages, described: next };
      changed = true;
    }
  }
  if (changed) version.value++;
}

function forwardPointerDown(e: PointerEvent, task: Task) {
  emit("pointer-down", e, task);
}
function forwardMoveTo(task: Task, quadrant: Quadrant) {
  emit("move-to", task, quadrant);
}
function forwardNudge(task: Task, delta: -1 | 1) {
  emit("nudge", task, delta);
}

function handle(): ColumnHandle | null {
  const el = body.value;
  return el ? { body: el, indexAtPointer, ensureVisible } : null;
}

/** Pointer y (viewport coords) to a slot index; null while the column renders every card. */
function indexAtPointer(clientY: number): number | null {
  const el = body.value;
  if (!el || !windowed.value) return null;
  const origin = el.getBoundingClientRect().top + el.clientTop - el.scrollTop;
  const placeholder = props.slotIndex == null
    ? undefined
    : { index: props.slotIndex, height: props.ghostHeight + CARD_GAP };
  return slotIndexAt(offsets.value, clientY - origin, placeholder);
}

async function ensureVisible(id: number) {
  const el = body.value;
  if (!el || !windowed.value) return;
  const index = props.rows.findIndex((task) => task.id === id);
  if (index < 0) return;
  const start = offsets.value[index] - LIST_PADDING;
  const end = offsets.value[index + 1] - LIST_PADDING;
  if (start < el.scrollTop || end > el.scrollTop + el.clientHeight) {
    el.scrollTop = Math.max(0, start - CARD_GAP);
    scrollTop.value = el.scrollTop;
    if (scrollTop.value > 0) scrolled.value = true;
    await nextTick();
  }
}

let observer: ResizeObserver | undefined;
onMounted(() => {
  const el = body.value;
  if (!el) return;
  viewport.value = el.clientHeight;
  observer = new ResizeObserver(() => {
    const next = el.clientHeight;
    if (next === viewport.value) return;
    viewport.value = next;
    // A width change reflows every card, so the measured heights have to be refreshed.
    heights.clear();
    version.value++;
    scheduleMeasure();
  });
  observer.observe(el);
  scheduleMeasure();
  emit("ready", handle());
});
onUpdated(scheduleMeasure);
onBeforeUnmount(() => {
  observer?.disconnect();
  if (scrollFrame) cancelAnimationFrame(scrollFrame);
  if (measureFrame) cancelAnimationFrame(measureFrame);
  emit("ready", null);
});
</script>

<template>
  <div
    ref="body"
    class="q-body"
    :title="`点击空白处新建任务（${QUADRANT_META[quadrant].name}）`"
    @click="emit('body-click', $event)"
    @scroll="onScroll"
  >
    <TransitionGroup tag="div" name="card" :css="false" class="q-list" role="list">
      <div v-if="topPad" key="top-pad" class="v-pad" :style="{ height: topPad + 'px' }" aria-hidden="true"></div>
      <template v-for="row in items" :key="row.key">
        <div
          v-if="row.slot"
          class="drop-slot"
          :style="{ height: ghostHeight + 'px' }"
          aria-hidden="true"
        ></div>
        <TaskCard
          v-else
          :task="row.task"
          :enter="enter"
          :now="row.task.due_at == null ? undefined : now"
          @toggle="emit('toggle', $event)"
          @remove="emit('remove', $event)"
          @pointer-down="forwardPointerDown"
          @move-to="forwardMoveTo"
          @nudge="forwardNudge"
          @edit="emit('edit', $event)"
        />
      </template>
      <div
        v-if="bottomPad"
        key="bottom-pad"
        class="v-pad"
        :style="{ height: bottomPad + 'px' }"
        aria-hidden="true"
      ></div>
    </TransitionGroup>
    <div v-if="!rows.length" class="empty">
      <svg width="28" height="28" viewBox="0 0 28 28" fill="none" stroke="currentColor" stroke-width="1.2">
        <rect x="4" y="4" width="20" height="20" rx="3" stroke-dasharray="3 3"/>
        <path d="M14 10v8M10 14h8" stroke-linecap="round"/>
      </svg>
      <span>点这里新建任务</span>
      <span class="empty-sub">也可以把卡片拖进来（键盘：选中卡片按 {{ quadrant }}）</span>
    </div>
  </div>
</template>

<style scoped>
/* Positioned so card offsetTop is measured from here (see useBoardDrag.slotIndex); the list
   inside must stay unpositioned, or it would become the offsetParent and drop that padding. */
.q-body {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 10px;
}
.v-pad { width: 100%; }
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  color: var(--text-muted);
  font-size: 12px;
  padding: 34px 0;
  opacity: 0.7;
  border-radius: var(--radius);
  transition: background var(--dur-fast) var(--ease), opacity var(--dur-fast) var(--ease);
}
/* 整块空白区域都是「新建任务」的入口，所以给一点可点的暗示 */
.empty:hover {
  background: var(--q-light);
  opacity: 1;
  color: var(--q-color);
}
.empty-sub { font-size: 11px; opacity: 0.75; }

/* Siblings glide out of the way (FLIP via TransitionGroup move class). */
.q-list > .card-move {
  transition: transform 0.2s var(--ease);
}
.drop-slot {
  box-sizing: border-box;
  margin-bottom: 8px;
  border: 1.5px dashed var(--q-color);
  border-radius: var(--radius);
  background: var(--q-light);
}
</style>
