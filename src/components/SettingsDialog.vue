<script setup lang="ts">
import { ref, onMounted, reactive } from "vue";
import { useTasks } from "../composables/useTasks";
import type { WebdavConfig } from "../types";

const emit = defineEmits<{ close: [] }>();
const {
  settings, saveApiKey, loadSettings,
  setTheme, exportData, importData,
  saveWebdav, syncToWebdav, restoreFromWebdav,
} = useTasks();

type Tab = "general" | "data" | "sync";
const activeTab = ref<Tab>("general");

const key = ref("");
const showKey = ref(false);
const saved = ref(false);
const theme = ref("light");

const webdav = reactive<WebdavConfig>({ url: "", username: "", password: "" });
const webdavSaved = ref(false);
const syncMsg = ref("");
const syncing = ref(false);

onMounted(async () => {
  await loadSettings();
  theme.value = settings.value.theme || "light";
});

async function saveKey() {
  if (!key.value.trim()) return;
  await saveApiKey(key.value.trim());
  saved.value = true;
  key.value = "";
  setTimeout(() => (saved.value = false), 2000);
}

async function switchTheme(t: string) {
  theme.value = t;
  await setTheme(t);
}

async function doExport() {
  const json = await exportData();
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `eisenhower-backup-${new Date().toISOString().slice(0, 10)}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

async function doImport(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const text = await file.text();
  try {
    const count = await importData(text);
    syncMsg.value = `成功导入 ${count} 个任务`;
    setTimeout(() => (syncMsg.value = ""), 3000);
  } catch (err: any) {
    syncMsg.value = `导入失败: ${err}`;
  }
  (e.target as HTMLInputElement).value = "";
}

async function saveWd() {
  if (!webdav.url.trim()) return;
  await saveWebdav({ ...webdav });
  webdavSaved.value = true;
  setTimeout(() => (webdavSaved.value = false), 2000);
}

async function doSync() {
  syncing.value = true;
  syncMsg.value = "";
  try {
    const r = await syncToWebdav();
    syncMsg.value = r.message;
  } catch (e: any) {
    syncMsg.value = `同步失败: ${e}`;
  } finally {
    syncing.value = false;
    setTimeout(() => (syncMsg.value = ""), 4000);
  }
}

async function doRestore() {
  syncing.value = true;
  syncMsg.value = "";
  try {
    const r = await restoreFromWebdav();
    syncMsg.value = r.message;
  } catch (e: any) {
    syncMsg.value = `恢复失败: ${e}`;
  } finally {
    syncing.value = false;
    setTimeout(() => (syncMsg.value = ""), 4000);
  }
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog anim-popIn">
      <div class="dialog-head">
        <div class="head-icon">
          <svg width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="10" cy="10" r="3"/>
            <path d="M10 2v2M10 16v2M2 10h2M16 10h2M4.5 4.5l1.4 1.4M14.1 14.1l1.4 1.4M4.5 15.5l1.4-1.4M14.1 5.9l1.4-1.4"/>
          </svg>
        </div>
        <div>
          <h3>管理设置</h3>
          <p class="head-sub">主题、数据与同步</p>
        </div>
        <button class="close-btn" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M3 3l8 8M11 3l-8 8"/>
          </svg>
        </button>
      </div>

      <div class="tabs">
        <button class="tab" :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">常规</button>
        <button class="tab" :class="{ active: activeTab === 'data' }" @click="activeTab = 'data'">数据</button>
        <button class="tab" :class="{ active: activeTab === 'sync' }" @click="activeTab = 'sync'">WebDAV 同步</button>
      </div>

      <div class="tab-content">
        <div v-if="activeTab === 'general'" class="pane">
          <div class="section">
            <label class="section-label">外观主题</label>
            <div class="theme-row">
              <button class="theme-btn" :class="{ active: theme === 'light' }" @click="switchTheme('light')">
                <span class="theme-preview light-preview"></span>浅色
              </button>
              <button class="theme-btn" :class="{ active: theme === 'dark' }" @click="switchTheme('dark')">
                <span class="theme-preview dark-preview"></span>深色
              </button>
            </div>
          </div>
          <div class="section">
            <label class="section-label">TypeSafe AI (JevAI) API Key</label>
            <div class="key-row">
              <input v-model="key" :type="showKey ? 'text' : 'password'" placeholder="sk-..." />
              <button class="btn btn-ghost btn-sm" @click="showKey = !showKey">显示</button>
            </div>
            <p class="hint">前往 <a href="https://dashboard.typesafe.ai" target="_blank">dashboard.typesafe.ai</a> 获取 Key。</p>
            <div class="status" :class="settings.api_key_configured ? 'ok' : 'warn'">
              <span class="dot"></span>{{ settings.api_key_configured ? "AI 分析已启用" : "未配置，AI 分析不可用" }}
            </div>
            <button class="btn btn-primary btn-sm" :disabled="!key.trim() || saved" @click="saveKey">
              {{ saved ? "已保存 ✓" : "保存 Key" }}
            </button>
          </div>
        </div>

        <div v-else-if="activeTab === 'data'" class="pane">
          <div class="section">
            <label class="section-label">导出备份</label>
            <p class="hint">将所有任务导出为 JSON 文件，可用于备份或迁移。</p>
            <button class="btn btn-primary" @click="doExport">导出 JSON</button>
          </div>
          <div class="section">
            <label class="section-label">导入恢复</label>
            <p class="hint">从 JSON 备份文件恢复任务（将覆盖当前所有数据）。</p>
            <label class="btn btn-ghost import-btn">
              <input type="file" accept=".json" @change="doImport" hidden />
              选择文件导入
            </label>
          </div>
        </div>

        <div v-else class="pane">
          <div class="section">
            <label class="section-label">WebDAV 服务器配置</label>
            <input v-model="webdav.url" type="text" placeholder="https://dav.example.com/eisenhower/backup.json" />
            <input v-model="webdav.username" type="text" placeholder="用户名" />
            <input v-model="webdav.password" type="password" placeholder="密码" />
            <div class="status" :class="settings.webdav_configured ? 'ok' : 'warn'">
              <span class="dot"></span>{{ settings.webdav_configured ? "WebDAV 已配置" : "未配置 WebDAV" }}
            </div>
            <button class="btn btn-primary btn-sm" :disabled="!webdav.url.trim() || webdavSaved" @click="saveWd">
              {{ webdavSaved ? "已保存 ✓" : "保存配置" }}
            </button>
          </div>
          <div class="section">
            <label class="section-label">同步操作</label>
            <div class="sync-row">
              <button class="btn btn-ghost" :disabled="syncing" @click="doSync">
                {{ syncing ? "同步中…" : "↑ 上传到 WebDAV" }}
              </button>
              <button class="btn btn-ghost" :disabled="syncing" @click="doRestore">
                {{ syncing ? "恢复中…" : "↓ 从 WebDAV 恢复" }}
              </button>
            </div>
            <p v-if="syncMsg" class="sync-msg">{{ syncMsg }}</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: 480px;
  max-height: 85vh;
  background: var(--surface);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}
.dialog-head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px 24px 16px;
}
.head-icon {
  width: 36px;
  height: 36px;
  background: var(--primary-light);
  border-radius: var(--radius);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--primary);
}
h3 { font-size: 16px; font-weight: 700; }
.head-sub { font-size: 12px; color: var(--text-muted); margin-top: 2px; }
.close-btn {
  margin-left: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
}
.close-btn:hover { background: var(--surface-2); color: var(--text); }
.tabs {
  display: flex;
  gap: 2px;
  padding: 0 24px;
  border-bottom: 1px solid var(--border-light);
}
.tab {
  padding: 10px 16px;
  font-size: 13px;
  color: var(--text-muted);
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  transition: all var(--dur-fast) var(--ease);
}
.tab:hover { color: var(--text); }
.tab.active {
  color: var(--primary);
  border-bottom-color: var(--primary);
  font-weight: 600;
}
.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 20px 24px;
}
.pane { display: flex; flex-direction: column; gap: 20px; }
.section { display: flex; flex-direction: column; gap: 8px; }
.section-label { font-size: 13px; font-weight: 600; color: var(--text-secondary); }
.theme-row { display: flex; gap: 10px; }
.theme-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border: 1.5px solid var(--border);
  border-radius: var(--radius);
  font-size: 13px;
  color: var(--text);
  transition: all var(--dur-fast) var(--ease);
}
.theme-btn:hover { border-color: var(--text-muted); }
.theme-btn.active {
  border-color: var(--primary);
  background: var(--primary-light);
  color: var(--primary);
}
.theme-preview {
  width: 24px;
  height: 16px;
  border-radius: 4px;
  border: 1px solid var(--border);
}
.light-preview { background: #fff; }
.dark-preview { background: #252830; }
.key-row { display: flex; gap: 8px; }
.hint { font-size: 12px; color: var(--text-muted); line-height: 1.5; }
.hint a { color: var(--primary); text-decoration: none; }
.hint a:hover { text-decoration: underline; }
.status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  padding: 7px 10px;
  border-radius: var(--radius-sm);
}
.status .dot { width: 6px; height: 6px; border-radius: 50%; }
.status.ok { background: var(--success-light); color: var(--success); }
.status.ok .dot { background: var(--success); }
.status.warn { background: var(--warning-light); color: var(--warning); }
.status.warn .dot { background: var(--warning); }
.import-btn { cursor: pointer; width: fit-content; }
.sync-row { display: flex; gap: 8px; flex-wrap: wrap; }
.sync-msg {
  font-size: 12px;
  color: var(--primary);
  padding: 8px 10px;
  background: var(--primary-light);
  border-radius: var(--radius-sm);
}
</style>
