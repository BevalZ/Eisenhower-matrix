<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import TaskCard from "./TaskCard.vue";
import TaskEditDialog from "./TaskEditDialog.vue";
import type { Task, Quadrant } from "../types";
import { QUADRANT_META } from "../types";
import { useTasks } from "../composables/useTasks";
import { useBoardDrag } from "../composables/useBoardDrag";
import { useDueReminders } from "../composables/useDueReminders";
import { clampIndex, naturalIndex, planDrop, type TaskMove } from "../ordering";

const emit = defineEmits<{ "new-task": [] }>();

const { tasks, toggleTaskDone, deleteTask, reorderTasks, holdRemoteRefresh, tasksByQuadrant } = useTasks();

const quadrants: Quadrant[] = [1, 2, 3, 4];
const layout: Record<Quadrant, { row: number; col: number }> = {
  1: { row: 0, col: 1 },
  2: { row: 0, col: 0 },
  3: { row: 1, col: 1 },
  4: { row: 1, col: 0 },
};

const { dragging, slot, outside, settling, ghostSize, ghostEl, setZone, setBody, onPointerDown } =
  useBoardDrag({
    list: tasksByQuadrant,
    commit: reorderTasks,
    lock: holdRemoteRefresh,
  });

type Row = { key: string; slot: true } | { key: string; slot: false; task: Task };

/** Cards per quadrant; while dragging, the dragged card is replaced by a placeholder at the drop slot. */
const rows = computed(() => {
  const out = {} as Record<Quadrant, Row[]>;
  const d = dragging.value;
  for (const q of quadrants) {
    const list: Row[] = tasksByQuadrant(q)
      .filter((t) => t.id !== d?.id)
      .map((task) => ({ key: `t${task.id}`, slot: false, task }));
    if (d && slot.value?.q === q) list.splice(slot.value.index, 0, { key: "drop-slot", slot: true });
    out[q] = list;
  }
  return out;
});

// ---- Keyboard moves (1-4 = quadrant, Alt+↑/↓ = order) ----

const liveMessage = ref("");
const editing = ref<Task | null>(null);

/** 点击象限空白处 = 新建任务（一律走 AI 向导，象限由 AI 判断，仍可在向导里微调）。 */
function onBodyClick(e: MouseEvent) {
  const target = e.target as HTMLElement | null;
  if (!target) return;
  // 只响应空白区域：象限容器本身，或「拖入任务」占位块；卡片与按钮不触发
  if (target === e.currentTarget || target.closest(".empty")) emit("new-task");
}

function closeEditor() {
  const id = editing.value?.id;
  editing.value = null;
  // Return focus to the card that was edited.
  void nextTick(() => document.querySelector<HTMLElement>(`[data-task-id="${id}"]`)?.focus());
}

function applyKeyboardMoves(task: Task, moves: TaskMove[], message: string) {
  if (!moves.length) return;
  void reorderTasks(moves);
  liveMessage.value = message;
  // The card is re-mounted (other quadrant) or re-inserted (new order); keep keyboard focus on it.
  void nextTick(() => {
    document.querySelector<HTMLElement>(`[data-task-id="${task.id}"]`)?.focus();
  });
}

function moveToQuadrant(task: Task, q: Quadrant) {
  if (task.quadrant === q) return;
  const list = tasksByQuadrant(q);
  const index = clampIndex(list, task.done, naturalIndex(list, task));
  applyKeyboardMoves(task, planDrop(task, q, list, index), `已移到「${QUADRANT_META[q].name}」第 ${index + 1} 位`);
}

// Deadlines: minute clock for the badges, one reminder when a deadline gets close.
const { now } = useDueReminders(tasks, (task, q) => {
  const current = tasks.value.find((t) => t.id === task.id);
  if (current) moveToQuadrant(current, q);
});

function nudge(task: Task, delta: -1 | 1) {
  const full = tasksByQuadrant(task.quadrant);
  const from = full.findIndex((t) => t.id === task.id);
  const list = full.filter((t) => t.id !== task.id);
  const to = clampIndex(list, task.done, from + delta);
  if (from < 0 || to === from) return;
  applyKeyboardMoves(task, planDrop(task, task.quadrant, list, to), `已移到第 ${to + 1} 位`);
}
</script>

<template>
  <div class="board" :class="{ dragging: !!dragging }">
    <div class="axis-axis">
      <span class="axis-label axis-y">↑ 重要</span>
      <div class="grid">
        <section
          v-for="q in quadrants"
          :key="q"
          :ref="(el) => setZone(q, el)"
          class="quadrant"
          :class="{ 'drop-target': !!dragging && !outside && slot?.q === q }"
          :style="{
            gridRow: layout[q].row + 1,
            gridColumn: layout[q].col + 1,
            '--q-color': QUADRANT_META[q].color,
            '--q-light': QUADRANT_META[q].bg,
          }"
          :aria-label="`${QUADRANT_META[q].name}，${tasksByQuadrant(q).length} 个任务`"
        >
          <div class="q-header">
            <span class="q-bar"></span>
            <div class="q-title-wrap">
              <span class="q-title">{{ QUADRANT_META[q].name }}</span>
              <span class="q-sub">{{ QUADRANT_META[q].subtitle }}</span>
            </div>
            <kbd class="q-key" :title="`选中卡片后按 ${q} 移到这里`">{{ q }}</kbd>
            <span class="q-count">{{ tasksByQuadrant(q).length }}</span>
          </div>
          <div
            :ref="(el) => setBody(q, el)"
            class="q-body"
            :title="`点击空白处新建任务（${QUADRANT_META[q].name}）`"
            @click="onBodyClick"
          >
            <TransitionGroup tag="div" name="card" :css="false" class="q-list" role="list">
              <template v-for="row in rows[q]" :key="row.key">
                <div
                  v-if="row.slot"
                  class="drop-slot"
                  :style="{ height: ghostSize.height + 'px' }"
                  aria-hidden="true"
                ></div>
                <TaskCard
                  v-else
                  :task="row.task"
                  @toggle="toggleTaskDone"
                  @remove="deleteTask"
                  @pointer-down="onPointerDown"
                  @move-to="moveToQuadrant"
                  @nudge="nudge"
                  :now="now"
                  @edit="editing = $event"
                />

              </template>
            </TransitionGroup>
            <div v-if="!rows[q].length" class="empty">
              <svg width="28" height="28" viewBox="0 0 28 28" fill="none" stroke="currentColor" stroke-width="1.2">
                <rect x="4" y="4" width="20" height="20" rx="3" stroke-dasharray="3 3"/>
                <path d="M14 10v8M10 14h8" stroke-linecap="round"/>
              </svg>
              <span>点这里新建任务</span>
              <span class="empty-sub">也可以把卡片拖进来（键盘：选中卡片按 {{ q }}）</span>
            </div>
          </div>
        </section>
      </div>
      <span class="axis-label axis-x">← 不紧急 &nbsp;&nbsp;|&nbsp;&nbsp; 紧急 →</span>
    </div>

    <!-- Drag ghost: a copy of the card that follows the pointer -->
    <Teleport to="body">
      <div
        v-if="dragging"
        ref="ghostEl"
        class="drag-ghost"
        :class="{ settling, outside }"
        :style="{ width: ghostSize.width + 'px', height: ghostSize.height + 'px' }"
      >
        <TaskCard :task="dragging" ghost />
      </div>
    </Teleport>

    <TaskEditDialog v-if="editing" :task="editing" @close="closeEditor" />

    <p id="board-keyboard-help" class="sr-only">
      按数字键 1 到 4 移到对应象限，Alt 加上下方向键调整顺序，Enter 编辑，空格切换完成，Delete 删除。
    </p>
    <div class="sr-only" aria-live="polite">{{ liveMessage }}</div>
  </div>
</template>

<style scoped>
.board {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 16px;
  overflow: hidden;
  position: relative;
}
.axis-axis {
  flex: 1;
  display: flex;
  flex-direction: column;
  position: relative;
  min-height: 0;
}
.axis-label {
  font-size: 11px;
  color: var(--text-muted);
  text-align: center;
  padding: 0 4px;
  letter-spacing: 0.5px;
}
.axis-y {
  position: absolute;
  left: -2px;
  top: 50%;
  transform: translateY(-50%) rotate(-90deg);
  transform-origin: left center;
  white-space: nowrap;
}
.axis-x { margin-top: 8px; }
.grid {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  gap: 12px;
  padding-left: 16px;
}
.quadrant {
  background: var(--surface);
  border: 1px solid var(--border-light);
  border-radius: var(--radius);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: border-color var(--dur-fast) var(--ease),
    box-shadow var(--dur-fast) var(--ease),
    background-color var(--dur-fast) var(--ease);
  box-shadow: var(--shadow-xs);
}
.quadrant.drop-target {
  border-color: var(--q-color);
  box-shadow: 0 0 0 3px var(--q-light), var(--shadow-xs);
}
.q-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-light);
  background: var(--surface-2);
}
.q-bar { width: 4px; height: 20px; border-radius: 2px; flex-shrink: 0; background: var(--q-color); }
.q-title-wrap { display: flex; flex-direction: column; gap: 1px; }
.q-title { font-weight: 600; font-size: 13px; line-height: 1.2; color: var(--q-color); }
.q-sub { font-size: 11px; color: var(--text-muted); }
.q-key {
  margin-left: auto;
  font-family: inherit;
  font-size: 10px;
  line-height: 1;
  padding: 3px 5px;
  border-radius: 4px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-muted);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease);
}
.board:focus-within .q-key { opacity: 1; }
.q-count {
  font-size: 11px;
  font-weight: 600;
  background: var(--surface);
  border: 1px solid var(--border-light);
  padding: 2px 9px;
  border-radius: 10px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
/* Positioned so card offsetTop is measured from here (see useBoardDrag.slotIndex). */
.q-body {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 10px;
}
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

.drag-ghost {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 150;
  pointer-events: none;
  will-change: transform;
  transition: opacity var(--dur-fast) var(--ease);
}
.drag-ghost > .task-card {
  height: 100%;
}
.drag-ghost.outside {
  opacity: 0.55;
}
.drag-ghost.settling {
  transition: transform 0.18s cubic-bezier(0.2, 0.8, 0.2, 1), opacity var(--dur-fast) var(--ease);
}
</style>
