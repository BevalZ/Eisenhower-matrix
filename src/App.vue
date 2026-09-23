<script setup lang="ts">
import { ref, onMounted } from "vue";
import QuadrantBoard from "./components/QuadrantBoard.vue";
import TaskWizard from "./components/TaskWizard.vue";
import StatsPanel from "./components/StatsPanel.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import { useTasks } from "./composables/useTasks";

const { loadTasks, loadSettings, clearDone, loading } = useTasks();

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
        <div class="logo-box">
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <rect x="1" y="1" width="8" height="8" rx="1.5" fill="var(--q1)" opacity="0.9"/>
            <rect x="11" y="1" width="8" height="8" rx="1.5" fill="var(--q2)" opacity="0.9"/>
            <rect x="1" y="11" width="8" height="8" rx="1.5" fill="var(--q3)" opacity="0.9"/>
            <rect x="11" y="11" width="8" height="8" rx="1.5" fill="var(--q4)" opacity="0.9"/>
          </svg>
        </div>
        <span class="brand-name">四象限任务管理器</span>
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
        <button class="btn btn-ghost btn-sm" @click="clearDone">
          清除已完成
        </button>
        <button class="icon-btn" @click="showSettings = true" title="设置">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="8" cy="8" r="2.5"/>
            <path d="M8 1v2M8 13v2M1 8h2M13 8h2M2.8 2.8l1.4 1.4M11.8 11.8l1.4 1.4M2.8 13.2l1.4-1.4M11.8 4.2l1.4-1.4"/>
          </svg>
        </button>
        <button class="btn btn-primary btn-sm" @click="showWizard = true">
          <span class="plus">+</span> 新建任务
        </button>
      </div>
    </header>

    <!-- Content -->
    <main class="content">
      <!-- Loading skeleton -->
      <div v-if="loading" class="loading-wrap">
        <div v-for="i in 4" :key="i" class="skeleton-col">
          <div class="skeleton" style="height: 28px; margin-bottom: 12px;"></div>
          <div v-for="j in 3" :key="j" class="skeleton" style="height: 60px; margin-bottom: 8px;"></div>
        </div>
      </div>

      <transition name="view-fade" mode="out-in">
        <QuadrantBoard v-if="view === 'board'" key="board" />
        <StatsPanel v-else key="stats" />
      </transition>
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
  height: 54px;
  background: var(--surface);
  border-bottom: 1px solid var(--border-light);
  -webkit-app-region: drag;
  z-index: 10;
  position: relative;
}
.titlebar button,
.titlebar .nav {
  -webkit-app-region: no-drag;
}
.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}
.logo-box {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--surface-2);
  border-radius: 8px;
}
.brand-name {
  font-weight: 600;
  font-size: 14.5px;
  letter-spacing: 0.2px;
}
.nav {
  display: flex;
  gap: 2px;
  margin-left: 8px;
  background: var(--surface-2);
  padding: 3px;
  border-radius: 8px;
}
.nav-btn {
  padding: 5px 16px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-muted);
  transition: all var(--dur-fast) var(--ease);
}
.nav-btn:hover {
  color: var(--text);
}
.nav-btn.active {
  background: var(--surface);
  color: var(--primary);
  font-weight: 600;
  box-shadow: var(--shadow-xs);
}
.actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 8px;
}
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
}
.icon-btn:hover {
  background: var(--surface-2);
  color: var(--text);
}
.plus {
  font-size: 16px;
  line-height: 1;
  margin-top: -1px;
}
.content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  position: relative;
}

/* View transition */
.view-fade-enter-active,
.view-fade-leave-active {
  transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease);
}
.view-fade-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.view-fade-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

/* Loading */
.loading-wrap {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  gap: 12px;
  padding: 16px;
}
.skeleton-col {
  padding: 12px;
}
</style>
