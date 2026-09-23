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
  await loadTasks();
  return task;
}

async function toggleTaskDone(id: number) {
  await invoke("toggle_task_done", { id });
  await loadTasks();
}

async function deleteTask(id: number) {
  await invoke("delete_task", { id });
  await loadTasks();
}

async function moveTask(id: number, quadrant: Quadrant, priority: number) {
  await invoke("move_task", { id, quadrant, priority });
  await loadTasks();
}

async function clearDone() {
  await invoke("clear_done_tasks");
  await loadTasks();
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
