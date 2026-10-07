<script setup lang="ts">
import { ref, onMounted, reactive, computed } from "vue";
import { useTasks } from "../composables/useTasks";
import { errorText } from "../composables/useToast";
import { usePlatform } from "../composables/usePlatform";
import type { PeerSyncStatus, TailscaleStatus, WebdavConfig } from "../types";

const emit = defineEmits<{ close: [] }>();
// 手机端没有 `tailscale` 命令，列不出其他设备，所以那边改成手动填对端地址
const { isMobile } = usePlatform();
const {
  settings, saveApiKey, saveAiConfig, loadSettings,
  setTheme, exportToFile, importData,
  saveWebdav, getWebdavConfig, syncToWebdav, restoreFromWebdav,
  getTailscaleStatus, getPeerSyncStatus, savePeerSyncConfig,
  setPeerSyncListening, syncWithPeer, syncAllPeers, revealSyncSecret, setSyncAiKey,
} = useTasks();

/** Inline status text that clears itself unless a newer message replaced it first. */
function flash(target: { value: string }, text: string, ms = 2000) {
  target.value = text;
  setTimeout(() => {
    if (target.value === text) target.value = "";
  }, ms);
}

type Tab = "general" | "data" | "sync" | "tailscale";
const activeTab = ref<Tab>("general");

const key = ref("");
const showKey = ref(false);
const saved = ref(false);
const theme = ref("light");

const webdav = reactive<WebdavConfig>({ url: "", username: "", password: "" });
const webdavHasPassword = ref(false);
const webdavSaved = ref(false);
const syncMsg = ref("");
const dataMsg = ref("");
const syncing = ref(false);
const tailscale = ref<TailscaleStatus | null>(null);
const peerSync = ref<PeerSyncStatus | null>(null);
const peerSecret = ref("");
const peerPort = ref(47321);
// 手机端列不出设备，手动填对端 Tailscale 地址
const peerIp = ref("");
const showPeerSecret = ref(false);
const peerMsg = ref("");
const peerBusy = ref(false);

const ai = reactive({ provider: "jevai" as "jevai" | "openai", base_url: "", model: "", api_key: "" });
const aiMsg = ref("");

async function saveAi() {
  aiMsg.value = "";
  try {
    await saveAiConfig({ ...ai });
    ai.api_key = "";
    flash(aiMsg, "已保存");
  } catch (err) {
    aiMsg.value = `保存失败: ${errorText(err)}`;
  }
}

function useOllama() {
  ai.base_url = "http://localhost:11434/v1";
  if (!ai.model) ai.model = "qwen2.5:7b";
}

onMounted(async () => {
  await loadSettings();
  theme.value = settings.value.theme || "light";
  ai.provider = settings.value.ai_provider ?? "jevai";
  ai.base_url = settings.value.ai_base_url ?? "";
  ai.model = settings.value.ai_model ?? "";

  try {
    const saved = await getWebdavConfig();
    webdav.url = saved.url;
    webdav.username = saved.username;
    webdavHasPassword.value = saved.has_password;
  } catch {
    /* form stays empty */
  }
});

// Basic Auth over plain http sends the password readable to anyone on the path.
const webdavInsecure = computed(() => {
  try {
    const u = new URL(webdav.url.trim());
    return u.protocol === "http:" && !["localhost", "127.0.0.1", "[::1]"].includes(u.hostname);
  } catch {
    return false;
  }
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
  try {
    const path = await exportToFile();
    if (path) dataMsg.value = `已导出到 ${path}`;
  } catch (err) {
    dataMsg.value = `导出失败: ${errorText(err)}`;
  }
}

async function doImport(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  if (!confirm("导入会覆盖当前所有任务，确定继续？")) {
    input.value = "";
    return;
  }
  const text = await file.text();
  try {
    const count = await importData(text);
    dataMsg.value = `成功导入 ${count} 个任务`;
    setTimeout(() => {
      if (dataMsg.value.startsWith("成功导入")) dataMsg.value = "";
    }, 3000);
  } catch (err) {
    dataMsg.value = `导入失败: ${errorText(err)}`;
  }
  input.value = "";
}

async function saveWd() {
  if (!webdav.url.trim()) return;
  try {
    await saveWebdav({ ...webdav });
  } catch (err) {
    syncMsg.value = `保存失败: ${errorText(err)}`;
    return;
  }
  if (webdav.password) webdavHasPassword.value = true;
  webdav.password = "";
  webdavSaved.value = true;
  setTimeout(() => (webdavSaved.value = false), 2000);
}

/** 是否把 AI 配置（含 API Key）跟着同步一起走。 */
async function toggleAiKeySync(enabled: boolean) {
  try {
    await setSyncAiKey(enabled);
    flash(syncMsg, enabled ? "已开启 AI 配置同步" : "已关闭 AI 配置同步");
  } catch (err) {
    syncMsg.value = `设置失败: ${errorText(err)}`;
  }
}

async function doSync() {
  syncing.value = true;
  syncMsg.value = "";
  try {
    const r = await syncToWebdav();
    syncMsg.value = r.message;
  } catch (e) {
    syncMsg.value = `同步失败: ${errorText(e)}`;
  } finally {
    syncing.value = false;
    setTimeout(() => (syncMsg.value = ""), 4000);
  }
}

function randomSecret() {
  const bytes = new Uint8Array(18);
  crypto.getRandomValues(bytes);
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function refreshTailscale() {
  peerSync.value = await getPeerSyncStatus();
  peerPort.value = peerSync.value.port || 47321;
  tailscale.value = await getTailscaleStatus();
}

// The saved secret stays in the backend until the user asks to see it; an empty input then
// means "keep the saved one", so saving the port never needs the secret typed again.
const canUsePeerSync = computed(
  () => !!peerSync.value?.secret_set || peerSecret.value.trim().length >= 8,
);

async function toggleSecret() {
  if (!peerSecret.value && peerSync.value?.secret_set) {
    try {
      peerSecret.value = await revealSyncSecret();
    } catch (err) {
      peerMsg.value = `读取已保存的密钥失败: ${errorText(err)}`;
      return;
    }
  }
  showPeerSecret.value = !showPeerSecret.value;
}

async function openTailscale() {
  activeTab.value = "tailscale";
  peerMsg.value = "";
  try {
    await refreshTailscale();
  } catch (err) {
    peerMsg.value = `读取 Tailscale 状态失败: ${errorText(err)}`;
  }
}

async function savePeer() {
  peerBusy.value = true;
  peerMsg.value = "";
  try {
    await savePeerSyncConfig(peerSecret.value.trim(), Number(peerPort.value));
    await refreshTailscale();
    // The value is saved; keep it out of the form until the user asks to see it again.
    peerSecret.value = "";
    showPeerSecret.value = false;
    flash(peerMsg, "同步配置已保存");
  } catch (err) {
    peerMsg.value = `保存失败: ${errorText(err)}`;
  } finally {
    peerBusy.value = false;
  }
}

async function togglePeerListen() {
  peerBusy.value = true;
  peerMsg.value = "";
  try {
    await savePeerSyncConfig(peerSecret.value.trim(), Number(peerPort.value));
    const addr = await setPeerSyncListening(!peerSync.value?.listening);
    await refreshTailscale();
    peerSecret.value = "";
    showPeerSecret.value = false;
    peerMsg.value = addr ? `已开始接受同步：${addr}` : "已停止接受同步";
  } catch (err) {
    peerMsg.value = `操作失败: ${errorText(err)}`;
    await refreshTailscale().catch(() => {});
  } finally {
    peerBusy.value = false;
  }
}

// The sync commands already reload the board before resolving, so don't load twice.
async function syncPeer(ip: string) {
  peerBusy.value = true;
  peerMsg.value = "";
  try {
    const result = await syncWithPeer(ip);
    peerMsg.value = result.message;
  } catch (err) {
    peerMsg.value = `同步失败: ${errorText(err)}`;
  } finally {
    peerBusy.value = false;
  }
}

async function syncPeers() {
  peerBusy.value = true;
  peerMsg.value = "";
  try {
    const result = await syncAllPeers();
    peerMsg.value = result.message;
  } catch (err) {
    peerMsg.value = `同步失败: ${errorText(err)}`;
  } finally {
    peerBusy.value = false;
  }
}

async function doRestore() {
  if (!confirm("「恢复」会丢弃本机所有任务，完全替换为 WebDAV 上的版本（日常同步请用「与 WebDAV 同步」）。确定继续？")) return;
  syncing.value = true;
  syncMsg.value = "";
  try {
    const r = await restoreFromWebdav();
    syncMsg.value = r.message;
  } catch (e) {
    syncMsg.value = `恢复失败: ${errorText(e)}`;
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
        <button class="close-btn" aria-label="关闭" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M3 3l8 8M11 3l-8 8"/>
          </svg>
        </button>
      </div>

      <div class="tabs">
        <button class="tab" :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">常规</button>
        <button class="tab" :class="{ active: activeTab === 'data' }" @click="activeTab = 'data'">数据</button>
        <button class="tab" :class="{ active: activeTab === 'sync' }" @click="activeTab = 'sync'">WebDAV</button>
        <button class="tab" :class="{ active: activeTab === 'tailscale' }" @click="openTailscale">多端</button>
      </div>

      <div class="tab-content">
        <div v-if="activeTab === 'general'" class="pane">
          <p v-if="settings.secrets_stored_in_plaintext" class="plaintext-warn" role="alert">
            ⚠ 系统凭据库不可用，API Key 与 WebDAV 密码目前以明文保存在本机数据库中，请勿在公共电脑上使用。
          </p>
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
            <label class="section-label">AI 服务</label>
            <div class="theme-row" role="radiogroup" aria-label="AI 服务">
              <button class="theme-btn" :class="{ active: ai.provider === 'jevai' }" role="radio" :aria-checked="ai.provider === 'jevai'" @click="ai.provider = 'jevai'; saveAi()">TypeSafe AI</button>
              <button class="theme-btn" :class="{ active: ai.provider === 'openai' }" role="radio" :aria-checked="ai.provider === 'openai'" @click="ai.provider = 'openai'">OpenAI 兼容 / Ollama</button>
            </div>
          </div>
          <div v-if="ai.provider === 'openai'" class="section">
            <label class="section-label">OpenAI 兼容接口</label>
            <input v-model="ai.base_url" type="text" placeholder="https://api.openai.com/v1" aria-label="接口地址" />
            <input v-model="ai.model" type="text" placeholder="模型名称，如 gpt-4o-mini、deepseek-chat、qwen2.5:7b" aria-label="模型名称" />
            <input
              v-model="ai.api_key"
              type="password"
              :placeholder="settings.openai_key_configured ? '已保存 Key（留空则不修改）' : 'API Key（本地 Ollama 可不填）'"
              aria-label="API Key"
            />
            <p class="hint">支持 OpenAI、DeepSeek、通义千问、Moonshot 等兼容接口。<button class="link-btn" @click="useOllama">使用本机 Ollama</button>：任务内容不离开这台电脑。</p>
            <div class="sync-row">
              <button class="btn btn-primary btn-sm" :disabled="!ai.base_url.trim() || !ai.model.trim()" @click="saveAi">保存</button>
              <span v-if="aiMsg" class="hint">{{ aiMsg }}</span>
            </div>
          </div>
          <div v-else class="section">
            <label class="section-label">TypeSafe AI (JevAI) API Key</label>
            <div class="key-row">
              <input v-model="key" :type="showKey ? 'text' : 'password'" placeholder="sk-..." />
              <button class="btn btn-ghost btn-sm" @click="showKey = !showKey">显示</button>
            </div>
            <p class="hint">前往 <a href="https://dashboard.typesafe.ai" target="_blank" rel="noopener">dashboard.typesafe.ai</a> 获取 Key。Key 保存在系统凭据管理器中。</p>
            <p class="hint">隐私提示：AI 分类会把你在向导里填写的任务内容发送给所选的 AI 服务；快速添加和手动选象限不会联网。想完全离线可以改用本机 Ollama。</p>
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
            <p v-if="dataMsg" class="sync-msg">{{ dataMsg }}</p>
          </div>
        </div>

        <div v-else-if="activeTab === 'sync'" class="pane">
          <div class="section">
            <label class="section-label">WebDAV 服务器配置</label>
            <input v-model="webdav.url" type="text" placeholder="https://dav.example.com/eisenhower/backup.json" aria-label="WebDAV 地址" />
            <input v-model="webdav.username" type="text" placeholder="用户名" aria-label="用户名" />
            <input
              v-model="webdav.password"
              type="password"
              :placeholder="webdavHasPassword ? '已保存密码（留空则不修改）' : '密码'"
              aria-label="密码"
            />
            <p v-if="webdavInsecure" class="hint warn-text">⚠ 这是 http 地址，密码会以明文传输。建议改用 https。</p>
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
                {{ syncing ? "同步中…" : "⇅ 与 WebDAV 同步" }}
              </button>
              <button class="btn btn-ghost" :disabled="syncing" @click="doRestore">
                {{ syncing ? "恢复中…" : "↓ 用 WebDAV 覆盖本机" }}
              </button>
            </div>
            <label class="save-opt">
              <input
                type="checkbox"
                :checked="settings.sync_ai_key !== false"
                @change="toggleAiKeySync(($event.target as HTMLInputElement).checked)"
              />
              同时同步 AI 配置与 API Key
            </label>
            <p class="hint">
              勾上以后，AI 服务、接口地址、模型和 API Key 会跟着任务一起同步，新设备不用再填一遍。
              注意：Key 是**明文**写进 WebDAV 文件里的，用公共网盘时请自行权衡。
            </p>
            <p v-if="syncMsg" class="sync-msg">{{ syncMsg }}</p>
          </div>
        </div>

        <div v-else class="pane">
          <div class="section">
            <label class="section-label">Tailscale 同步</label>
            <p class="hint">两端登录同一个 tailnet，填写相同密钥和端口，并都点击「开始接受同步」。之后大约每 45 秒自动对齐，也可以手动同步。任务按修改时间合并，不会整库覆盖。系统时间需要大致准确。</p>
            <div class="status" :class="tailscale?.running ? 'ok' : 'warn'">
              <span class="dot"></span>
              <span v-if="tailscale?.running">本机 {{ tailscale.hostname }} · {{ tailscale.ip }}</span>
              <span v-else>{{ tailscale?.message || "尚未检查 Tailscale" }}</span>
            </div>
            <button class="btn btn-ghost btn-sm" :disabled="peerBusy" @click="refreshTailscale">刷新设备</button>
          </div>
          <div class="section">
            <label class="section-label">同步密钥与端口</label>
            <div class="key-row">
              <input
                v-model="peerSecret"
                :type="showPeerSecret ? 'text' : 'password'"
                :placeholder="peerSync?.secret_set ? '已保存密钥（留空则不修改）' : '两端相同的密钥'"
              />
              <button class="btn btn-ghost btn-sm" title="显示已保存的密钥，或切换明文显示" @click="toggleSecret">显示</button>
              <button class="btn btn-ghost btn-sm" @click="peerSecret = randomSecret()">生成</button>
            </div>
            <input v-model.number="peerPort" type="number" min="1024" max="65535" placeholder="47321" />
            <p class="hint">默认端口 47321。Tailscale ACL 需要允许设备之间访问这个 TCP 端口。</p>
            <p class="hint">密钥只保存在本机，输入框留空表示继续使用已保存的密钥；点「显示」可以读出当前密钥，方便填到另一台设备。</p>
            <div class="sync-row">
              <button class="btn btn-primary btn-sm" :disabled="peerBusy || !canUsePeerSync" @click="savePeer">保存</button>
              <button class="btn btn-ghost btn-sm" :disabled="peerBusy || !canUsePeerSync" @click="togglePeerListen">
                {{ peerSync?.listening ? "停止接受同步" : "开始接受同步" }}
              </button>
            </div>
            <p v-if="peerSync?.listening" class="hint">正在监听 {{ peerSync.address }}，并自动同步在线设备。</p>
            <p v-if="peerSync?.last_error" class="hint">{{ peerSync.last_error }}</p>
          </div>
          <div class="section">
            <label class="section-label">在线设备</label>
            <p class="hint">第一次同步前，历史任务会各自生成 uid。如果两边本来就有任务，可能出现重复，删掉多余的即可。也可以先用 JSON 把一台设备的数据导入另一台。</p>
            <!-- 手机端列不出设备：手动填对端 Tailscale 地址 -->
            <template v-if="isMobile">
              <p class="hint">手机端无法自动列出设备，请在上方设备状态里查看本机地址，并在这里填写对方的 Tailscale 地址（100.x.y.z）。</p>
              <div class="key-row">
                <input v-model="peerIp" type="text" placeholder="对方 Tailscale 地址，如 100.64.0.5" aria-label="对方 Tailscale 地址" />
                <button class="btn btn-primary btn-sm" :disabled="peerBusy || !peerIp.trim()" @click="syncPeer(peerIp.trim())">同步</button>
              </div>
            </template>
            <template v-else>
              <div v-if="!tailscale?.peers.length" class="hint">没有发现其他 Tailscale 设备。</div>
              <div v-else class="peer-list">
                <div v-for="peer in tailscale.peers" :key="peer.ip" class="peer">
                  <div>
                    <div class="peer-name">{{ peer.hostname }}</div>
                    <div class="peer-meta">{{ peer.ip }} · {{ peer.os || "未知系统" }} · {{ peer.online ? "在线" : "离线" }}</div>
                  </div>
                  <button class="btn btn-ghost btn-sm" :disabled="peerBusy || !peer.online" @click="syncPeer(peer.ip)">同步</button>
                </div>
              </div>
              <button class="btn btn-primary btn-sm" :disabled="peerBusy || !tailscale?.peers.some((peer) => peer.online)" @click="syncPeers">
                {{ peerBusy ? "同步中…" : "同步所有在线设备" }}
              </button>
            </template>
          </div>
          <p v-if="peerMsg" class="sync-msg peer-msg">{{ peerMsg }}</p>
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
  width: 560px;
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
.plaintext-warn {
  font-size: 12px;
  line-height: 1.6;
  color: var(--danger);
  background: var(--danger-light);
  border-radius: var(--radius-sm);
  padding: 9px 11px;
}
.hint a { color: var(--primary); text-decoration: none; }
.hint a:hover { text-decoration: underline; }
.hint.warn-text { color: var(--warning); }
.link-btn { color: var(--primary); padding: 0; font-size: inherit; }
.link-btn:hover { text-decoration: underline; }
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
  white-space: pre-wrap;
}
.peer-list { display: flex; flex-direction: column; gap: 8px; }
.peer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid var(--border-light);
  border-radius: var(--radius-sm);
}
.peer-name { font-size: 13px; font-weight: 600; }
.peer-meta { font-size: 11px; color: var(--text-muted); }
</style>

/* 复选框行（例如「同时同步 AI 配置与 API Key」） */
.save-opt {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text-secondary);
  cursor: pointer;
}
.save-opt input {
  accent-color: var(--primary);
}
