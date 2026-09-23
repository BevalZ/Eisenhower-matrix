<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import TaskCard from "./TaskCard.vue";
import type { Task, Quadrant } from "../types";
import { QUADRANT_META } from "../types";
import { useTasks } from "../composables/useTasks";

const { toggleTaskDone, deleteTask, moveTask, tasksByQuadrant } = useTasks();

const dragOverQuadrant = ref<Quadrant | null>(null);
const draggingTask = ref<Task | null>(null);
const dragPos = ref({ x: 0, y: 0 });
let dragStartX = 0, dragStartY = 0;
let isPotentialDrag = false;

function onCardPointerDown(e: PointerEvent, task: Task) {
  if (e.button !== 0) return;
  isPotentialDrag = true;
  dragStartX = e.clientX;
  dragStartY = e.clientY;
  dragPos.value = { x: e.clientX, y: e.clientY };
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
}

function onPointerMove(e: PointerEvent) {
  if (!isPotentialDrag) return;
  const dx = e.clientX - dragStartX;
  const dy = e.clientY - dragStartY;
  // Threshold: only start drag after moving 5px
  if (!draggingTask.value && Math.hypot(dx, dy) < 5) return;

  dragPos.value = { x: e.clientX, y: e.clientY };

  // Find which quadrant element is under the pointer
  const el = document.elementFromPoint(e.clientX, e.clientY);
  const quadrantEl = el?.closest("[data-quadrant]") as HTMLElement | null;
  if (quadrantEl) {
    dragOverQuadrant.value = Number(quadrantEl.dataset.quadrant) as Quadrant;
  } else {
    dragOverQuadrant.value = null;
  }
}

async function onPointerUp(e: PointerEvent) {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  isPotentialDrag = false;

  if (!draggingTask.value) return;
  const task = draggingTask.value;

  // Find target quadrant
  const el = document.elementFromPoint(e.clientX, e.clientY);
  const quadrantEl = el?.closest("[data-quadrant]") as HTMLElement | null;
  if (quadrantEl) {
    const q = Number(quadrantEl.dataset.quadrant) as Quadrant;
    if (q !== task.quadrant) {
      const list = tasksByQuadrant(q);
      const maxP = list.length ? Math.max(...list.map((t) => t.priority)) : 0;
      await moveTask(task.id, q, Math.min(100, maxP + 5));
    }
  }

  draggingTask.value = null;
  dragOverQuadrant.value = null;
}

function startDrag(task: Task) {
  draggingTask.value = task;
}

onUnmounted(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
});

const quadrants: Quadrant[] = [1, 2, 3, 4];
const layout: Record<Quadrant, { row: number; col: number }> = {
  1: { row: 0, col: 1 },
  2: { row: 0, col: 0 },
  3: { row: 1, col: 1 },
  4: { row: 1, col: 0 },
};
</script>

<template>
  <div class="board">
    <div class="axis-axis">
      <span class="axis-label axis-y">↑ 重要</span>
      <div class="grid">
        <div
          v-for="q in quadrants"
          :key="q"
          class="quadrant"
          :data-quadrant="q"
          :class="{ 'drag-over': dragOverQuadrant === q }"
          :style="{
            gridRow: layout[q].row + 1,
            gridColumn: layout[q].col + 1,
          }"
        >
          <div class="q-header">
            <span class="q-bar" :style="{ background: QUADRANT_META[q].color }"></span>
            <div class="q-title-wrap">
              <span class="q-title" :style="{ color: QUADRANT_META[q].color }">{{ QUADRANT_META[q].name }}</span>
              <span class="q-sub">{{ QUADRANT_META[q].subtitle }}</span>
            </div>
            <span class="q-count">{{ tasksByQuadrant(q).length }}</span>
          </div>
          <div class="q-body">
            <TaskCard
              v-for="task in tasksByQuadrant(q)"
              :key="task.id"
              :task="task"
              @toggle="toggleTaskDone"
              @remove="deleteTask"
              @drag-start="startDrag"
              @pointer-down="onCardPointerDown"
            />
            <div v-if="!tasksByQuadrant(q).length" class="empty">
              <svg width="28" height="28" viewBox="0 0 28 28" fill="none" stroke="currentColor" stroke-width="1.2">
                <rect x="4" y="4" width="20" height="20" rx="3" stroke-dasharray="3 3"/>
                <path d="M14 10v8M10 14h8" stroke-linecap="round"/>
              </svg>
              <span>拖入任务</span>
            </div>
          </div>
        </div>
      </div>
      <span class="axis-label axis-x">← 不紧急 &nbsp;&nbsp;|&nbsp;&nbsp; 紧急 →</span>
    </div>

    <!-- Drag ghost -->
    <div
      v-if="draggingTask"
      class="drag-ghost"
      :style="{ left: dragPos.x + 'px', top: dragPos.y + 'px' }"
    >
      {{ draggingTask.title }}
    </div>
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
    box-shadow var(--dur-fast) var(--ease);
  box-shadow: var(--shadow-xs);
}
.quadrant.drag-over {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-light);
}
.q-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-light);
  background: var(--surface-2);
}
.q-bar { width: 4px; height: 20px; border-radius: 2px; flex-shrink: 0; }
.q-title-wrap { display: flex; flex-direction: column; gap: 1px; }
.q-title { font-weight: 600; font-size: 13px; line-height: 1.2; }
.q-sub { font-size: 11px; color: var(--text-muted); }
.q-count {
  margin-left: auto;
  font-size: 11px;
  font-weight: 600;
  background: var(--surface);
  border: 1px solid var(--border-light);
  padding: 2px 9px;
  border-radius: 10px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
.q-body { flex: 1; overflow-y: auto; padding: 10px; }
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: var(--text-muted);
  font-size: 12px;
  padding: 40px 0;
  opacity: 0.6;
}
.drag-ghost {
  position: fixed;
  pointer-events: none;
  z-index: 9999;
  background: var(--primary);
  color: white;
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  box-shadow: 0 4px 20px rgba(0,0,0,0.2);
  transform: translate(-50%, -50%);
  max-width: 250px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
