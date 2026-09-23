import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Task, TaskInput, ClassificationResult, StatsSummary, Settings, Quadrant } from "../types";

const tasks = ref<Task[]>([]);
const loading = ref(false);
const settings = ref<Settings>({ api_key_configured: false });

async function loadTasks() {
  loading.value = true;
  try {
    tasks.value = await invoke<Task[]>("get_tasks");
  } finally {
    loading.value = false;
  }
}

async function loadSettings() {
  settings.value = await invoke<Settings>("get_settings");
}

async function saveApiKey(key: string) {
  await invoke("set_api_key", { key });
  await loadSettings();
}

async function classifyTask(description: string): Promise<ClassificationResult> {
  return await invoke<ClassificationResult>("classify_task", { description });
}

async function createTask(input: TaskInput): Promise<Task> {
  const task = await invoke<Task>("create_task", { input });
  // Optimistic: append locally without full reload
  tasks.value = [...tasks.value, task];
  return task;
}

async function toggleTaskDone(id: number) {
  // Optimistic update
  const task = tasks.value.find((t) => t.id === id);
  if (task) {
    task.done = !task.done;
    task.completed_at = task.done ? Date.now() : null;
  }
  try {
    await invoke("toggle_task_done", { id });
  } catch {
    // Rollback on failure
    if (task) task.done = !task.done;
  }
}

async function deleteTask(id: number) {
  // Optimistic: remove locally
  const idx = tasks.value.findIndex((t) => t.id === id);
  if (idx >= 0) tasks.value.splice(idx, 1);
  try {
    await invoke("delete_task", { id });
  } catch {
    await loadTasks(); // Reload on failure
  }
}

async function moveTask(id: number, quadrant: Quadrant, priority: number) {
  // Optimistic: update locally
  const task = tasks.value.find((t) => t.id === id);
  if (task) {
    task.quadrant = quadrant;
    task.priority = priority;
  }
  try {
    await invoke("move_task", { id, quadrant, priority });
  } catch {
    await loadTasks();
  }
}

async function clearDone() {
  // Optimistic: remove done tasks
  tasks.value = tasks.value.filter((t) => !t.done);
  try {
    await invoke("clear_done_tasks");
  } catch {
    await loadTasks();
  }
}

async function getStats(): Promise<StatsSummary> {
  return await invoke<StatsSummary>("get_stats");
}

const sortedTasks = computed(() => {
  return [...tasks.value].sort((a, b) => {
    if (a.done !== b.done) return a.done ? 1 : -1;
    return b.priority - a.priority;
  });
});

function tasksByQuadrant(q: Quadrant): Task[] {
  return sortedTasks.value.filter((t) => t.quadrant === q);
}

export function useTasks() {
  return {
    tasks,
    loading,
    settings,
    loadTasks,
    loadSettings,
    saveApiKey,
    classifyTask,
    createTask,
    toggleTaskDone,
    deleteTask,
    moveTask,
    clearDone,
    getStats,
    tasksByQuadrant,
  };
}
