<script setup lang="ts">
import { computed, ref } from "vue";
import type { Quadrant, Task } from "../types";
import { QUADRANT_META } from "../types";

const props = defineProps<{ task: Task; ghost?: boolean }>();
const emit = defineEmits<{
  toggle: [id: number];
  remove: [id: number];
  "pointer-down": [e: PointerEvent, task: Task];
  "move-to": [task: Task, quadrant: Quadrant];
  nudge: [task: Task, delta: -1 | 1];
  edit: [task: Task];
}>();

function onDblClick(e: MouseEvent) {
  if (!(e.target as HTMLElement).closest("button")) emit("edit", props.task);
}

const meta = computed(() => QUADRANT_META[props.task.quadrant]);

// Entrance animation only on mount: re-inserting a node during reorder would replay a
// class-based animation and make every sibling flicker.
const entering = ref(true);
function onAnimationEnd(e: AnimationEvent) {
  if (e.target === e.currentTarget) entering.value = false;
}
const label = computed(
  () =>
    `${props.task.title}，${meta.value.name}，优先级 ${Math.round(props.task.priority)}${props.task.done ? "，已完成" : ""}`,
);

function onPointerDown(e: PointerEvent) {
  if ((e.target as HTMLElement).closest("button")) return;
  emit("pointer-down", e, props.task);
}

function onKeyDown(e: KeyboardEvent) {
  if (e.target !== e.currentTarget || e.ctrlKey || e.metaKey) return;
  if (!e.altKey && ["1", "2", "3", "4"].includes(e.key)) {
    e.preventDefault();
    emit("move-to", props.task, Number(e.key) as Quadrant);
  } else if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
    e.preventDefault();
    emit("nudge", props.task, e.key === "ArrowUp" ? -1 : 1);
  } else if (!e.altKey && e.key === "Enter") {
    e.preventDefault();
    emit("edit", props.task);
  } else if (!e.altKey && e.key === " ") {
    e.preventDefault();
    emit("toggle", props.task.id);
  } else if (!e.altKey && (e.key === "Delete" || e.key === "Backspace")) {
    e.preventDefault();
    emit("remove", props.task.id);
  }
}
</script>

<template>
  <div
    v-if="ghost"
    class="task-card ghost"
    :class="{ done: task.done }"
    :style="{ '--q-color': meta.color, '--q-light': meta.bg }"
    aria-hidden="true"
  >
    <div class="accent"></div>
    <div class="card-body">
      <div class="card-top">
        <span class="check" :class="{ checked: task.done }"></span>
        <span class="title">{{ task.title }}</span>
      </div>
      <p v-if="task.description" class="desc">{{ task.description }}</p>
      <div class="meta-row">
        <span class="tag"><span class="dot"></span>{{ meta.name }}</span>
        <span class="score">{{ Math.round(task.priority) }}</span>
      </div>
    </div>
  </div>
  <div
    v-else
    class="task-card"
    :data-task-id="task.id"
    :class="{ done: task.done, entering }"
    :style="{ '--q-color': meta.color, '--q-light': meta.bg }"
    role="listitem"
    tabindex="0"
    :aria-label="label"
    aria-describedby="board-keyboard-help"
    @pointerdown="onPointerDown"
    @keydown="onKeyDown"
    @dblclick="onDblClick"
    @animationend="onAnimationEnd"
  >
    <div class="accent"></div>
    <div class="card-body">
      <div class="card-top">
        <button
          class="check"
          :class="{ checked: task.done }"
          :aria-label="task.done ? '标记为未完成' : '标记为已完成'"
          :aria-pressed="task.done"
          @click="emit('toggle', task.id)"
        >
          <svg v-if="task.done" width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path d="M1.5 5.5L4 8L8.5 2.5" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <span class="title" :title="task.title">{{ task.title }}</span>
        <button class="del edit" aria-label="编辑任务" title="编辑（Enter / 双击）" @click="emit('edit', task)">
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round">
            <path d="M8.2 1.8l2 2L4 10H2V8z"/>
          </svg>
        </button>
        <button class="del" aria-label="删除任务" @click="emit('remove', task.id)">
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
  touch-action: none;
  user-select: none;
  -webkit-user-select: none;
  outline: none;
}
.task-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-1px);
}
.task-card:focus-visible {
  border-color: var(--q-color);
  box-shadow: 0 0 0 3px var(--q-light), var(--shadow-sm);
}
.task-card:active { cursor: grabbing; }
.task-card.done { opacity: 0.5; }
.task-card.done .title {
  text-decoration: line-through;
  color: var(--text-muted);
}
.task-card.ghost {
  margin: 0;
  cursor: grabbing;
  box-shadow: var(--shadow-lg);
  border-color: var(--q-color);
}
.task-card.ghost.done { opacity: 0.75; }
.accent {
  position: absolute;
  left: 0; top: 0; bottom: 0;
  width: 3px;
  background: var(--q-color);
  opacity: 0.7;
}
.task-card.entering { animation: cardIn 0.16s var(--ease-spring); }
@keyframes cardIn {
  from { opacity: 0.6; transform: scale(0.96); }
  to { opacity: 1; transform: scale(1); }
}
.card-body { padding: 10px 12px 10px 14px; }
/* min-height keeps the ghost (no delete button) exactly as tall as the card */
.card-top { display: flex; align-items: center; gap: 8px; min-height: 22px; }
.title {
  flex: 1;
  font-weight: 500;
  font-size: 13.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.check {
  width: 18px; height: 18px;
  border-radius: 50%;
  border: 1.5px solid var(--border);
  display: flex; align-items: center; justify-content: center;
  flex-shrink: 0;
  transition: all var(--dur-fast) var(--ease-spring);
  background: var(--surface);
}
.check:hover { border-color: var(--success); }
.check.checked { background: var(--success); border-color: var(--success); transform: scale(1.05); }
.check:focus-visible,
.del:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.del {
  display: flex; align-items: center; justify-content: center;
  width: 22px; height: 22px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease), background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease);
}
.task-card:hover .del,
.task-card:focus-within .del { opacity: 1; }
.del:hover { background: var(--danger-light); color: var(--danger); }
.del.edit:hover { background: var(--primary-light); color: var(--primary); }
.desc {
  font-size: 12px; color: var(--text-secondary);
  margin: 6px 0 0 26px; line-height: 1.5;
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
}
.meta-row { display: flex; justify-content: space-between; align-items: center; margin-top: 8px; margin-left: 26px; }
.tag {
  display: inline-flex; align-items: center; gap: 4px;
  font-size: 11px; padding: 2px 8px; border-radius: 10px;
  font-weight: 500; color: var(--q-color); background: var(--q-light);
}
.tag .dot { width: 5px; height: 5px; border-radius: 50%; background: var(--q-color); }
.score { font-size: 11px; color: var(--text-muted); }
</style>
