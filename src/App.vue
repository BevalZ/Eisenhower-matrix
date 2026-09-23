<script setup lang="ts">
import { ref, onMounted } from "vue";
import QuadrantBoard from "./components/QuadrantBoard.vue";
import TaskWizard from "./components/TaskWizard.vue";
import StatsPanel from "./components/StatsPanel.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import { useTasks } from "./composables/useTasks";

const { loadTasks, loadSettings, clearDone } = useTasks();

const view = ref<"board" | "stats">("board");
const showWizard = ref(false);
const showSettings = ref(false);

onMounted(async () => {
  await Promise.all([loadTasks(), loadSettings()]);
});
</script>

<template>
  <div class="app">
    <!-- Title bar -->
    <header class="titlebar">
      <div class="brand">
        <span class="logo">⊞</span>
        <span>四象限任务管理器</span>
      </div>
      <nav class="nav">
        <button
          class="nav-btn"
          :class="{ active: view === 'board' }"
          @click="view = 'board'"
        >
          看板
        </button>
        <button
          class="nav-btn"
          :class="{ active: view === 'stats' }"
          @click="view = 'stats'"
        >
          统计
        </button>
      </nav>
      <div class="actions">
        <button class="btn btn-ghost btn-sm" @click="clearDone">清除已完成</button>
        <button class="icon-btn" @click="showSettings = true" title="设置">⚙</button>
        <button class="btn btn-primary btn-sm" @click="showWizard = true">+ 新建任务</button>
      </div>
    </header>

    <!-- Content -->
    <main class="content">
      <QuadrantBoard v-if="view === 'board'" />
      <StatsPanel v-else />
    </main>

    <!-- Modals -->
    <TaskWizard v-if="showWizard" @close="showWizard = false" />
    <SettingsDialog v-if="showSettings" @close="showSettings = false" />
  </div>
</template>

<style scoped>
.app {
  height: 100%;
  display: flex;
  flex-direction: column;
}
.titlebar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 16px;
  height: 52px;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  -webkit-app-region: drag;
}
.titlebar button,
.titlebar .nav {
  -webkit-app-region: no-drag;
}
.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 15px;
}
.logo {
  font-size: 18px;
  color: var(--primary);
}
.nav {
  display: flex;
  gap: 4px;
  margin-left: 12px;
}
.nav-btn {
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-muted);
  transition: all 0.15s;
}
.nav-btn:hover {
  background: var(--bg);
  color: var(--text);
}
.nav-btn.active {
  background: rgba(49, 130, 206, 0.1);
  color: var(--primary);
  font-weight: 500;
}
.actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 8px;
}
.icon-btn {
  font-size: 16px;
  padding: 6px 8px;
  border-radius: 6px;
  color: var(--text-muted);
}
.icon-btn:hover {
  background: var(--bg);
}
.content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>
