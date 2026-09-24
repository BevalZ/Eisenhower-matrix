import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  quadrantFromScores,
  scoresForQuadrant,
  type Task, type TaskInput, type ClassificationResult, type StatsSummary, type Settings,
  type Quadrant, type WebdavConfig, type SyncResult, type LearningStats,
  type TailscaleStatus, type PeerSyncStatus,
} from "../types";

interface QuadrantCorrection {
  taskTitle: string;
  aiImportance: number;
  aiUrgency: number;
  aiQuadrant: number;
  userImportance: number;
  userUrgency: number;
  userQuadrant: Quadrant;
}

const tasks = ref<Task[]>([]);
const loading = ref(false);
const settings = ref<Settings>({ api_key_configured: false, theme: "light", webdav_configured: false });

async function loadTasks(options?: { quiet?: boolean }) {
  const quiet = options?.quiet ?? false;
  if (!quiet) loading.value = true;
  try {
    tasks.value = await invoke<Task[]>("get_tasks");
  } finally {
    if (!quiet) loading.value = false;
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
  const task = tasks.value.find((t) => t.id === id);
  if (!task) return;
  const prevDone = task.done;
  const prevCompleted = task.completed_at;
  task.done = !task.done;
  task.completed_at = task.done ? Date.now() : null;
  try {
    await invoke("toggle_task_done", { id });
  } catch {
    task.done = prevDone;
    task.completed_at = prevCompleted;
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

function recordQuadrantCorrection(input: QuadrantCorrection) {
  invoke("record_feedback", { ...input }).catch(() => {});
}

async function moveTask(id: number, quadrant: Quadrant, priority: number) {
  const task = tasks.value.find((t) => t.id === id);
  if (!task) return;
  const prevQuadrant = task.quadrant;
  const prevPriority = task.priority;
  const scoresMatchOrigin =
    quadrantFromScores(task.importance_score, task.urgency_score) === prevQuadrant;
  task.quadrant = quadrant;
  task.priority = priority;
  try {
    await invoke("move_task", { id, quadrant, priority });
    if (scoresMatchOrigin && prevQuadrant !== quadrant) {
      const user = scoresForQuadrant(task.importance_score, task.urgency_score, quadrant);
      recordQuadrantCorrection({
        taskTitle: task.title,
        aiImportance: task.importance_score,
        aiUrgency: task.urgency_score,
        aiQuadrant: prevQuadrant,
        userImportance: user.importance,
        userUrgency: user.urgency,
        userQuadrant: quadrant,
      });
    }
  } catch {
    task.quadrant = prevQuadrant;
    task.priority = prevPriority;
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

async function getTailscaleStatus(): Promise<TailscaleStatus> {
  return await invoke<TailscaleStatus>("tailscale_status");
}

async function getPeerSyncStatus(): Promise<PeerSyncStatus> {
  return await invoke<PeerSyncStatus>("peer_sync_status");
}

async function savePeerSyncConfig(secret: string, port: number) {
  await invoke("save_peer_sync", { config: { secret, port } });
}

async function setPeerSyncListening(enabled: boolean): Promise<string> {
  return await invoke<string>("set_peer_sync_listening", { enabled });
}

async function syncWithPeer(ip: string): Promise<SyncResult> {
  const result = await invoke<SyncResult>("sync_with_peer", { ip });
  await loadTasks();
  return result;
}

async function syncAllPeers(): Promise<SyncResult> {
  const result = await invoke<SyncResult>("sync_all_peers");
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

let remoteWatchStarted = false;

function watchRemoteTasks() {
  if (remoteWatchStarted) return;
  remoteWatchStarted = true;
  void listen("tasks-changed", () => {
    void loadTasks({ quiet: true }).catch(() => {});
  });
}

export function useTasks() {
  watchRemoteTasks();
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
    recordQuadrantCorrection,
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
    getTailscaleStatus,
    getPeerSyncStatus,
    savePeerSyncConfig,
    setPeerSyncListening,
    syncWithPeer,
    syncAllPeers,
    tasksByQuadrant,
  };
}
