import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createRefreshQueue } from "../refreshQueue";
import { applyTaskDelta, type TaskCursor, type TaskDelta } from "../taskDelta";
import {
  isUnscored,
  quadrantFromScores,
  scoresForQuadrant,
  type AiConfig, type Task, type TaskInput, type TaskUpdate,
 type ClassificationResult, type StatsSummary, type Settings,
  type Quadrant, type WebdavConfig, type WebdavInfo, type SyncResult, type LearningStats,
  type TailscaleStatus, type PeerSyncStatus,
} from "../types";
import { compareTasks, type TaskMove } from "../ordering";
import { useToast } from "./useToast";

const toast = useToast();

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
const loading = ref(true);
const tasksLoaded = ref(false);
const settings = ref<Settings>({ api_key_configured: false, theme: "light", webdav_configured: false });
const taskIndex = computed(() => new Map(tasks.value.map((task) => [task.id, task])));

// Incremental reads: the board asks for what changed since the last answer instead of the
// whole table. Refreshes after a bulk command drop the cursor and take a full snapshot again,
// and every so often one is taken anyway, so a missed delta can never stick.
const FULL_RESYNC_EVERY = 20;
let cursor: TaskCursor | null = null;
let deltasSinceFull = 0;

function invalidateTaskCache() {
  cursor = null;
  deltasSinceFull = 0;
}

function applyTasks(delta: TaskDelta) {
  tasks.value = applyTaskDelta(tasks.value, delta);
  cursor = { generation: delta.generation, watermark: delta.watermark };
  deltasSinceFull = delta.full ? 0 : deltasSinceFull + 1;
  tasksLoaded.value = true;
}

const refresh = createRefreshQueue(
  () => {
    const resume = deltasSinceFull >= FULL_RESYNC_EVERY ? null : cursor;
    return invoke<TaskDelta>("get_tasks_delta", {
      generation: resume?.generation ?? null,
      watermark: resume?.watermark ?? null,
    });
  },
  applyTasks,
);
let loadingRequests = 0;

async function loadTasks(options?: { quiet?: boolean }) {
  const quiet = options?.quiet ?? false;
  if (!quiet) { loadingRequests++; loading.value = true; }
  try {
    await refresh.request();
  } finally {
    if (!quiet) loading.value = --loadingRequests > 0;
  }
}

// Reads started before a write must not overwrite its optimistic update.
let pendingWrite: Promise<unknown> = Promise.resolve();
/** Bulk commands rewrite rows a delta cannot describe, so take a full snapshot instead. */
async function reloadTasksFully() {
  invalidateTaskCache();
  await loadTasks();
}

async function writeTasks<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const release = refresh.pause();
  // Native commands run on worker threads; preserve the user's write order.
  const result = pendingWrite.then(() => invoke<T>(command, args));
  pendingWrite = result.catch(() => {});
  try {
    return await result;
  } finally {
    release();
  }
}

async function loadSettings() {
  settings.value = await invoke<Settings>("get_settings");
}

async function saveApiKey(key: string) {
  await invoke("set_api_key", { key });
  await loadSettings();
}

async function saveAiConfig(config: AiConfig) {
  await invoke("save_ai_config", { config });
  await loadSettings();
}

/** 是否让 AI 配置（含 API Key）跟着 WebDAV / 多端同步一起走。 */
async function setSyncAiKey(enabled: boolean) {
  await invoke("set_sync_ai_key", { enabled });
  await loadSettings();
}

async function classifyTask(description: string): Promise<ClassificationResult> {
  return await invoke<ClassificationResult>("classify_task", { description });
}

async function createTask(input: TaskInput): Promise<Task> {
  const task = await writeTasks<Task>("create_task", { input });
  // Optimistic: append locally without full reload
  tasks.value = [...tasks.value, task];
  return task;
}

async function toggleTaskDone(id: number) {
  const task = taskIndex.value.get(id);
  if (!task) return;
  const prevDone = task.done;
  const prevCompleted = task.completed_at;
  task.done = !task.done;
  task.completed_at = task.done ? Date.now() : null;
  try {
    await writeTasks("toggle_task_done", { id });
  } catch (err) {
    task.done = prevDone;
    task.completed_at = prevCompleted;
    toast.showError("更新任务失败", err);
  }
}

function shortTitle(title: string) {
  return title.length > 16 ? `${title.slice(0, 16)}…` : title;
}

async function deleteTask(id: number) {
  // Optimistic: remove locally, offer undo (deletion is a tombstone, so it can be restored)
  const idx = tasks.value.findIndex((t) => t.id === id);
  if (idx < 0) return;
  const [removed] = tasks.value.splice(idx, 1);
  try {
    await writeTasks("delete_task", { id });
  } catch (err) {
    tasks.value.splice(Math.min(idx, tasks.value.length), 0, removed);
    toast.showError("删除失败", err);
    return;
  }
  toast.show(`已删除「${shortTitle(removed.title)}」`, {
    action: { label: "撤销", run: () => restoreTasks([id]) },
  });
}

async function restoreTasks(ids: number[]) {
  try {
    await writeTasks<number>("restore_tasks", { ids });
    invalidateTaskCache();
    await loadTasks({ quiet: true });
  } catch (err) {
    toast.showError("撤销失败", err);
  }
}

function recordQuadrantCorrection(input: QuadrantCorrection) {
  invoke("record_feedback", { ...input }).catch(() => {});
}

/**
 * Apply a drag/keyboard move. The local update happens synchronously so the board
 * re-renders in the same tick; all rows are saved in one transaction.
 */
async function reorderTasks(moves: TaskMove[]) {
  const changes = moves.flatMap((move) => {
    const task = taskIndex.value.get(move.id);
    return task ? [{ task, move, prevQuadrant: task.quadrant, prevPriority: task.priority }] : [];
  });
  if (!changes.length) return;
  for (const { task, move } of changes) {
    task.quadrant = move.quadrant;
    task.priority = move.priority;
  }
  try {
    await writeTasks("reorder_tasks", { moves: changes.map((c) => c.move) });
  } catch (err) {
    for (const { task, prevQuadrant, prevPriority } of changes) {
      task.quadrant = prevQuadrant;
      task.priority = prevPriority;
    }
    toast.showError("移动失败，已恢复原位置", err);
    return;
  }
  for (const { task, move, prevQuadrant } of changes) learnFromMove(task, prevQuadrant, move.quadrant);
}

/** AI learning: a first move out of the AI-chosen quadrant counts as a correction. */
function learnFromMove(task: Task, from: Quadrant, to: Quadrant) {
  if (from === to || isUnscored(task)) return;
  if (quadrantFromScores(task.importance_score, task.urgency_score) !== from) return;
  const user = scoresForQuadrant(task.importance_score, task.urgency_score, to);
  recordQuadrantCorrection({
    taskTitle: task.title,
    aiImportance: task.importance_score,
    aiUrgency: task.urgency_score,
    aiQuadrant: from,
    userImportance: user.importance,
    userUrgency: user.urgency,
    userQuadrant: to,
  });
}

async function updateTask(input: TaskUpdate) {
  const task = taskIndex.value.get(input.id);
  if (!task) return;
  // Snapshot every field the write touches, so a failed save restores all of them.
  const prev = {
    title: task.title,
    description: task.description,
    quadrant: task.quadrant,
    priority: task.priority,
    due_at: task.due_at,
  };
  Object.assign(task, { ...input, title: input.title.trim() });
  try {
    await writeTasks("update_task", { input });
  } catch (err) {
    Object.assign(task, prev);
    throw err;
  }
  learnFromMove(task, prev.quadrant, input.quadrant);
}

// Remote sync refreshes are deferred while a card is being dragged,
// otherwise a reload could swap the list out from under the pointer.
const holdRemoteRefresh = refresh.pause;

async function getLearningStats(): Promise<LearningStats> {
  return await invoke<LearningStats>("get_learning_stats");
}

async function clearDone() {
  if (!tasks.value.some((t) => t.done)) {
    toast.show("没有已完成的任务");
    return;
  }
  // Optimistic: remove done tasks, offer undo
  tasks.value = tasks.value.filter((t) => !t.done);
  try {
    const ids = await writeTasks<number[]>("clear_done_tasks");
    toast.show(`已清除 ${ids.length} 个已完成任务`, {
      action: { label: "撤销", run: () => restoreTasks(ids) },
    });
  } catch (err) {
    toast.showError("清除失败", err);
    invalidateTaskCache();
    await loadTasks({ quiet: true }).catch(() => {});
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
  // main.ts reads this before mounting, so dark-theme users don't see a light flash on start.
  try {
    localStorage.setItem("theme", theme);
  } catch {
    /* storage unavailable */
  }
}

async function exportData(): Promise<string> {
  return await invoke<string>("export_data");
}

/** Native save dialog; resolves to the saved path, or null if cancelled. */
async function exportToFile(): Promise<string | null> {
  return await invoke<string | null>("export_to_file");
}

async function importData(json: string): Promise<number> {
  const count = await writeTasks<number>("import_data", { json });
  await reloadTasksFully();
  return count;
}

async function saveWebdav(config: WebdavConfig) {
  await invoke("save_webdav", { config });
  await loadSettings();
}

async function getWebdavConfig(): Promise<WebdavInfo> {
  return await invoke<WebdavInfo>("get_webdav_config");
}

async function syncToWebdav(): Promise<SyncResult> {
  const result = await invoke<SyncResult>("sync_to_webdav");
  // 同步可能带回 AI 配置（含 Key），所以设置也要重读
  await Promise.all([reloadTasksFully(), loadSettings()]);
  return result;
}

async function restoreFromWebdav(): Promise<SyncResult> {
  const result = await invoke<SyncResult>("restore_from_webdav");
  await Promise.all([reloadTasksFully(), loadSettings()]);
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

/** Read the saved shared secret back; the settings page asks for it on demand only. */
async function revealSyncSecret(): Promise<string> {
  return await invoke<string>("reveal_sync_secret");
}

async function setPeerSyncListening(enabled: boolean): Promise<string> {
  return await invoke<string>("set_peer_sync_listening", { enabled });
}

async function syncWithPeer(ip: string): Promise<SyncResult> {
  const result = await invoke<SyncResult>("sync_with_peer", { ip });
  await Promise.all([reloadTasksFully(), loadSettings()]);
  return result;
}

async function syncAllPeers(): Promise<SyncResult> {
  const result = await invoke<SyncResult>("sync_all_peers");
  await Promise.all([reloadTasksFully(), loadSettings()]);
  return result;
}

const sortedTasks = computed(() => [...tasks.value].sort(compareTasks));

/** Board order, grouped once per change instead of re-filtering the list on every read. */
const byQuadrant = computed<Record<Quadrant, Task[]>>(() => {
  const out: Record<Quadrant, Task[]> = { 1: [], 2: [], 3: [], 4: [] };
  for (const task of sortedTasks.value) out[task.quadrant].push(task);
  return out;
});

/** Read-only view of one quadrant in board order; never mutate the returned array. */
function tasksByQuadrant(q: Quadrant): Task[] {
  return byQuadrant.value[q];
}

export function useTasks() {
  return {
    tasks,
    tasksLoaded,
    loading,
    settings,
    loadTasks,
    loadSettings,
    saveApiKey,
    saveAiConfig,
    setSyncAiKey,
    classifyTask,
    createTask,
    updateTask,
    toggleTaskDone,
    deleteTask,
    restoreTasks,
    reorderTasks,
    holdRemoteRefresh,
    recordQuadrantCorrection,
    getLearningStats,
    clearDone,
    getStats,
    setTheme,
    applyTheme,
    exportData,
    exportToFile,
    importData,
    saveWebdav,
    getWebdavConfig,
    syncToWebdav,
    restoreFromWebdav,
    getTailscaleStatus,
    getPeerSyncStatus,
    savePeerSyncConfig,
    revealSyncSecret,
    setPeerSyncListening,
    syncWithPeer,
    syncAllPeers,
    tasksByQuadrant,
  };
}
