<script setup lang="ts">
import { ref } from "vue";
import TaskCard from "./TaskCard.vue";
import type { Task, Quadrant } from "../types";
import { QUADRANT_META } from "../types";
import { useTasks } from "../composables/useTasks";

const { toggleTaskDone, deleteTask, moveTask, tasksByQuadrant } = useTasks();

const dragOverQuadrant = ref<Quadrant | null>(null);
const draggingTask = ref<Task | null>(null);

function onDragStart(task: Task) {
  draggingTask.value = task;
}
function onDragEnd() {
  draggingTask.value = null;
  dragOverQuadrant.value = null;
}
function onDragOver(e: DragEvent, q: Quadrant) {
  e.preventDefault();
  dragOverQuadrant.value = q;
}
function onDragLeave(q: Quadrant) {
  if (dragOverQuadrant.value === q) dragOverQuadrant.value = null;
}
async function onDrop(e: DragEvent, q: Quadrant) {
  e.preventDefault();
  const id = Number(e.dataTransfer?.getData("text/plain"));
  if (!id || !draggingTask.value) return;

  // Determine new priority: place at top of target quadrant
  const list = tasksByQuadrant(q);
  const maxP = list.length ? Math.max(...list.map((t) => t.priority)) : 0;
  const newPriority = draggingTask.value.quadrant === q
    ? draggingTask.value.priority
    : Math.min(100, maxP + 5);

  await moveTask(id, q, newPriority);
  onDragEnd();
}

const quadrants: Quadrant[] = [1, 2, 3, 4];
// Layout: [Q1 Q2] / [Q3 Q4] — Q1 top-right (important+urgent), Q2 top-left
const layout: Record<Quadrant, { row: number; col: number }> = {
  1: { row: 0, col: 1 },
  2: { row: 0, col: 0 },
  3: { row: 1, col: 1 },
  4: { row: 1, col: 0 },
};
</script>

<template>
  <div class="board">
    <div class="axis-label axis-y">重要 →</div>
    <div class="grid">
      <div
        v-for="q in quadrants"
        :key="q"
        class="quadrant"
        :style="{
          gridRow: layout[q].row + 1,
          gridColumn: layout[q].col + 1,
          background: QUADRANT_META[q].bg,
          borderColor:
            dragOverQuadrant === q ? QUADRANT_META[q].color : QUADRANT_META[q].border,
        }"
        @dragover="onDragOver($event, q)"
        @dragleave="onDragLeave(q)"
        @drop="onDrop($event, q)"
      >
        <div class="q-header">
          <span class="q-title" :style="{ color: QUADRANT_META[q].color }">
            {{ QUADRANT_META[q].name }}
          </span>
          <span class="q-sub">{{ QUADRANT_META[q].subtitle }}</span>
          <span class="q-count">{{ tasksByQuadrant(q).length }}</span>
        </div>
        <div class="q-body">
          <TaskCard
            v-for="task in tasksByQuadrant(q)"
            :key="task.id"
            :task="task"
            @toggle="toggleTaskDone"
            @remove="deleteTask"
            @dragstart="onDragStart"
            @dragend="onDragEnd"
          />
          <div v-if="!tasksByQuadrant(q).length" class="empty">拖拽任务到这里</div>
        </div>
      </div>
    </div>
    <div class="axis-label axis-x">不紧急 ←—— 紧急 →</div>
  </div>
</template>

<style scoped>
.board {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 16px;
  gap: 8px;
  overflow: hidden;
}
.axis-label {
  font-size: 11px;
  color: var(--text-muted);
  text-align: center;
  padding: 0 8px;
}
.axis-y {
  writing-mode: vertical-rl;
  transform: rotate(180deg);
  position: absolute;
  left: 4px;
  top: 50%;
  transform: translateY(-50%) rotate(180deg);
}
.grid {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  gap: 12px;
  position: relative;
}
.quadrant {
  border: 2px dashed var(--border);
  border-radius: var(--radius);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: border-color 0.15s;
}
.q-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
}
.q-title {
  font-weight: 600;
  font-size: 13px;
}
.q-sub {
  font-size: 11px;
  color: var(--text-muted);
}
.q-count {
  margin-left: auto;
  font-size: 11px;
  background: var(--surface);
  padding: 2px 8px;
  border-radius: 10px;
  color: var(--text-muted);
}
.q-body {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
}
.empty {
  text-align: center;
  color: var(--text-muted);
  font-size: 12px;
  padding: 30px 0;
}
</style>
