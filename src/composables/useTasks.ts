import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  Task, TaskInput, ClassificationResult, StatsSummary, Settings,
  Quadrant, WebdavConfig, SyncResult, LearningStats,
} from "../types";

const tasks = ref<Task[]>([]);
const loading = ref(false);
const settings = ref<Settings>({ api_key_configured: false, theme: "light", webdav_configured: false });

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
  // Capture AI original scores before update for feedback
  const task = tasks.value.find((t) => t.id === id);
  const prevQuadrant = task?.quadrant;
  const prevImportance = task?.importance_score ?? 2.5;
  const prevUrgency = task?.urgency_score ?? 2.5;
  const taskTitle = task?.title ?? "";

  // Optimistic: update locally
  if (task) {
    task.quadrant = quadrant;
    task.priority = priority;
    // Update scores to reflect new quadrant
    task.importance_score = quadrant <= 2 ? 3.0 : 2.0;
    task.urgency_score = quadrant === 1 || quadrant === 3 ? 3.0 : 2.0;
  }
  try {
    await invoke("move_task", { id, quadrant, priority });
    // Record feedback if user moved to a different quadrant than AI assigned
    if (prevQuadrant !== undefined && prevQuadrant !== quadrant) {
      invoke("record_feedback", {
        taskTitle,
        aiImportance: prevImportance,
        aiUrgency: prevUrgency,
        aiQuadrant: prevQuadrant,
        userImportance: task?.importance_score ?? 2.5,
        userUrgency: task?.urgency_score ?? 2.5,
        userQuadrant: quadrant,
      }).catch(() => {}); // Fire and forget, don't block
    }
  } catch {
    await loadTasks();
  }
}

async function getLearningStats(): Promise<LearningStats> {
  return await invoke<LearningStats>("get_learning_stats");
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

async function setTheme(theme: string) {
  await invoke("set_theme", { theme });
  applyTheme(theme);
}

function applyTheme(theme: string) {
  document.documentElement.setAttribute("data-theme", theme);
}

async function exportData(): Promise<string> {
  return await invoke<string>("export_data");
}

async function importData(json: string): Promise<number> {
  const count = await invoke<number>("import_data", { json });
  await loadTasks();
  return count;
}

async function saveWebdav(config: WebdavConfig) {
  await invoke("save_webdav", { config });
  await loadSettings();
}

async function syncToWebdav(): Promise<SyncResult> {
  return await invoke<SyncResult>("sync_to_webdav");
}

async function restoreFromWebdav(): Promise<SyncResult> {
  const result = await invoke<SyncResult>("restore_from_webdav");
  await loadTasks();
  return result;
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
    getLearningStats,
    clearDone,
    getStats,
    setTheme,
    applyTheme,
    exportData,
    importData,
    saveWebdav,
    syncToWebdav,
    restoreFromWebdav,
    tasksByQuadrant,
  };
}
