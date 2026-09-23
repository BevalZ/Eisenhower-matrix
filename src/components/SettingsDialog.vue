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
  setTimeout(() => (saved.value = false), 2000);
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3>设置</h3>
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
          获取 API Key。配置后即可使用 AI 自动判断任务象限。
        </p>
        <p v-if="settings.api_key_configured" class="status ok">✓ 已配置 API Key</p>
        <p v-else class="status warn">⚠ 尚未配置，AI 分析功能不可用</p>
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
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: 420px;
  background: var(--surface);
  border-radius: 14px;
  padding: 22px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.2);
}
h3 {
  margin-bottom: 16px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
label {
  font-size: 13px;
  font-weight: 500;
}
.key-row {
  display: flex;
  gap: 8px;
}
.hint {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.5;
}
.hint a {
  color: var(--primary);
}
.status {
  font-size: 12px;
  margin-top: 4px;
}
.status.ok {
  color: var(--success);
}
.status.warn {
  color: #d69e2e;
}
.footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 18px;
}
</style>
