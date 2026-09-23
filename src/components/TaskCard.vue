<script setup lang="ts">
import type { Task } from "../types";
import { QUADRANT_META } from "../types";

const props = defineProps<{ task: Task }>();
const emit = defineEmits<{
  toggle: [id: number];
  remove: [id: number];
  dragstart: [task: Task];
  dragend: [];
}>();

function onDragStart(e: DragEvent) {
  e.dataTransfer?.setData("text/plain", String(props.task.id));
  e.dataTransfer!.effectAllowed = "move";
  emit("dragstart", props.task);
}

const meta = QUADRANT_META[props.task.quadrant];
</script>

<template>
  <div
    class="task-card anim-popIn"
    :class="{ done: task.done }"
    :style="{ '--q-color': meta.color, '--q-light': meta.bg }"
    draggable="true"
    @dragstart="onDragStart"
    @dragend="emit('dragend')"
  >
    <div class="accent"></div>
    <div class="card-body">
      <div class="card-top">
        <button
          class="check"
          :class="{ checked: task.done }"
          @click="emit('toggle', task.id)"
        >
          <svg v-if="task.done" width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path d="M1.5 5.5L4 8L8.5 2.5" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <span class="title">{{ task.title }}</span>
        <button class="del" @click="emit('remove', task.id)">
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M3 3l6 6M9 3l-6 6"/>
          </svg>
        </button>
      </div>
      <p v-if="task.description" class="desc">{{ task.description }}</p>
      <div class="meta-row">
        <span class="tag">
          <span class="dot"></span>{{ meta.name }}
        </span>
        <span class="score">{{ Math.round(task.priority) }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.task-card {
  position: relative;
  background: var(--surface);
  border: 1px solid var(--border-light);
  border-radius: var(--radius);
  margin-bottom: 8px;
  cursor: grab;
  transition: box-shadow var(--dur-fast) var(--ease),
    opacity var(--dur) var(--ease),
    transform var(--dur-fast) var(--ease);
  overflow: hidden;
}
.task-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-1px);
}
.task-card:active {
  cursor: grabbing;
  transform: scale(0.98);
}
.task-card.done {
  opacity: 0.5;
}
.task-card.done .title {
  text-decoration: line-through;
  color: var(--text-muted);
}

/* Left accent bar */
.accent {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: var(--q-color);
  opacity: 0.7;
}

.card-body {
  padding: 10px 12px 10px 14px;
}

.card-top {
  display: flex;
  align-items: center;
  gap: 8px;
}
.title {
  flex: 1;
  font-weight: 500;
  font-size: 13.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: color var(--dur) var(--ease);
}

/* Checkbox */
.check {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1.5px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all var(--dur-fast) var(--ease-spring);
  background: var(--surface);
}
.check:hover {
  border-color: var(--success);
}
.check.checked {
  background: var(--success);
  border-color: var(--success);
  transform: scale(1.05);
}

/* Delete */
.del {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease),
    background var(--dur-fast) var(--ease),
    color var(--dur-fast) var(--ease);
}
.task-card:hover .del {
  opacity: 1;
}
.del:hover {
  background: var(--danger-light);
  color: var(--danger);
}

.desc {
  font-size: 12px;
  color: var(--text-secondary);
  margin: 6px 0 0 26px;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.meta-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
  margin-left: 26px;
}
.tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  font-weight: 500;
  color: var(--q-color);
  background: var(--q-light);
}
.tag .dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--q-color);
}
.score {
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
