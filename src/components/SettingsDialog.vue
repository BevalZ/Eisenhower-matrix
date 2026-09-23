<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useTasks } from "../composables/useTasks";

const emit = defineEmits<{ close: [] }>();
const { settings, saveApiKey, loadSettings } = useTasks();

const key = ref("");
const show = ref(false);
const saved = ref(false);

onMounted(loadSettings);

async function save() {
  if (!key.value.trim()) return;
  await saveApiKey(key.value.trim());
  saved.value = true;
  key.value = "";
  setTimeout(() => (saved.value = false), 2500);
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog anim-popIn">
      <div class="dialog-head">
        <div class="head-icon">
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="10" cy="10" r="3"/>
            <path d="M10 2v2M10 16v2M2 10h2M16 10h2M4.5 4.5l1.4 1.4M14.1 14.1l1.4 1.4M4.5 15.5l1.4-1.4M14.1 5.9l1.4-1.4"/>
          </svg>
        </div>
        <div>
          <h3>设置</h3>
          <p class="head-sub">配置 AI 服务连接</p>
        </div>
        <button class="close-btn" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M3 3l8 8M11 3l-8 8"/>
          </svg>
        </button>
      </div>

      <div class="field">
        <label>TypeSafe AI (JevAI) API Key</label>
        <div class="key-row">
          <input
            v-model="key"
            :type="show ? 'text' : 'password'"
            placeholder="sk-..."
          />
          <button class="btn btn-ghost btn-sm" @click="show = !show">
            {{ show ? "隐藏" : "显示" }}
          </button>
        </div>
        <p class="hint">
          前往
          <a href="https://dashboard.typesafe.ai" target="_blank">dashboard.typesafe.ai</a>
          获取 API Key。配置后 AI 将自动分析任务的重要性与紧急性。
        </p>
        <div class="status" :class="settings.api_key_configured ? 'ok' : 'warn'">
          <span class="status-dot"></span>
          {{ settings.api_key_configured ? "已配置 API Key，AI 分析可用" : "尚未配置，AI 分析功能不可用" }}
        </div>
      </div>

      <div class="footer">
        <button class="btn btn-ghost" @click="emit('close')">关闭</button>
        <button class="btn btn-primary" :disabled="!key.trim() || saved" @click="save">
          {{ saved ? "已保存 ✓" : "保存" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(29, 33, 41, 0.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: 440px;
  background: var(--surface);
  border-radius: var(--radius-lg);
  padding: 24px;
  box-shadow: var(--shadow-lg);
}
.dialog-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
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
h3 {
  font-size: 16px;
  font-weight: 700;
  line-height: 1.2;
}
.head-sub {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 2px;
}
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
.close-btn:hover {
  background: var(--surface-2);
  color: var(--text);
}
.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
label {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
}
.key-row {
  display: flex;
  gap: 8px;
}
.hint {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.6;
}
.hint a {
  color: var(--primary);
  text-decoration: none;
}
.hint a:hover {
  text-decoration: underline;
}
.status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  margin-top: 4px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.status.ok {
  background: var(--success-light);
  color: var(--success);
}
.status.ok .status-dot {
  background: var(--success);
}
.status.warn {
  background: var(--warning-light);
  color: var(--warning);
}
.status.warn .status-dot {
  background: var(--warning);
}
.footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 20px;
}
</style>
