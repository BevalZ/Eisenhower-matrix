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
    class="task-card"
    :class="{ done: task.done }"
    draggable="true"
    @dragstart="onDragStart"
    @dragend="emit('dragend')"
  >
    <div class="card-top">
      <button
        class="check"
        :class="{ checked: task.done }"
        @click="emit('toggle', task.id)"
      >
        <span v-if="task.done">✓</span>
      </button>
      <span class="title">{{ task.title }}</span>
      <button class="del" @click="emit('remove', task.id)">✕</button>
    </div>
    <p v-if="task.description" class="desc">{{ task.description }}</p>
    <div class="meta-row">
      <span class="tag" :style="{ color: meta.color, background: meta.bg }">
        {{ meta.name }}
      </span>
      <span class="score">优先级 {{ Math.round(task.priority) }}</span>
    </div>
  </div>
</template>

<style scoped>
.task-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 8px;
  cursor: grab;
  transition: box-shadow 0.15s, opacity 0.15s;
}
.task-card:hover {
  box-shadow: var(--shadow-md);
}
.task-card.done {
  opacity: 0.55;
}
.task-card.done .title {
  text-decoration: line-through;
}
.card-top {
  display: flex;
  align-items: center;
  gap: 8px;
}
.title {
  flex: 1;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.check {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: #fff;
  flex-shrink: 0;
  transition: all 0.15s;
}
.check.checked {
  background: var(--success);
  border-color: var(--success);
}
.del {
  color: var(--text-muted);
  font-size: 14px;
  padding: 2px 4px;
  border-radius: 4px;
  opacity: 0;
  transition: opacity 0.15s;
}
.task-card:hover .del {
  opacity: 1;
}
.del:hover {
  color: var(--danger);
}
.desc {
  font-size: 12px;
  color: var(--text-muted);
  margin: 6px 0 0 26px;
  line-height: 1.4;
}
.meta-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
  margin-left: 26px;
}
.tag {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  font-weight: 500;
}
.score {
  font-size: 11px;
  color: var(--text-muted);
}
</style>
