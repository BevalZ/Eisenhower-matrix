<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useTasks } from "../composables/useTasks";
import { QUADRANT_META, type StatsSummary, type Quadrant } from "../types";

const { getStats } = useTasks();
const stats = ref<StatsSummary | null>(null);
const loading = ref(true);
const error = ref("");
let unlisten: UnlistenFn | undefined;

async function refreshStats(quiet = false) {
  if (!quiet) loading.value = true;
  error.value = "";
  try {
    stats.value = await getStats();
  } catch (e) {
    if (!quiet || !stats.value) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  unlisten = await listen("tasks-changed", () => {
    void refreshStats(true);
  });
  await refreshStats();
});

onUnmounted(() => {
  unlisten?.();
});

const quadrants: Quadrant[] = [1, 2, 3, 4];
</script>

<template>
  <div class="stats-panel">
    <div class="panel-header">
      <h2>统计概览</h2>
      <p class="sub">你的任务完成情况一览</p>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="loading-grid">
      <div v-for="i in 4" :key="i" class="skeleton" style="height: 90px; border-radius: 10px;"></div>
      <div class="skeleton" style="height: 60px; grid-column: span 4;"></div>
    </div>

    <p v-else-if="error" class="stats-error">统计加载失败：{{ error }}</p>

    <div v-else-if="stats" class="grid">
      <!-- Overall cards -->
      <div class="stat-card anim-slideUp" v-for="(item, i) in [
        { num: stats.total, label: '总任务', color: 'var(--text)' },
        { num: stats.done, label: '已完成', color: 'var(--success)' },
        { num: stats.pending, label: '待处理', color: 'var(--danger)' },
        { num: Math.round(stats.completion_rate * 100) + '%', label: '完成率', color: 'var(--primary)' },
      ]" :key="i" :style="{ animationDelay: i * 0.06 + 's' }">
        <span class="num" :style="{ color: item.color }">{{ item.num }}</span>
        <span class="lbl">{{ item.label }}</span>
      </div>

      <!-- Completion rate bar -->
      <div class="full rate-section">
        <div class="rate-label">
          <span>整体完成进度</span>
          <span class="rate-pct">{{ Math.round(stats.completion_rate * 100) }}%</span>
        </div>
        <div class="bar-wrap">
          <div class="bar-fill" :style="{ width: stats.completion_rate * 100 + '%' }"></div>
        </div>
      </div>

      <!-- Per-quadrant -->
      <div
        v-for="q in quadrants"
        :key="q"
        class="q-stat anim-slideUp"
        :style="{ animationDelay: (4 + q) * 0.06 + 's' }"
      >
        <div class="q-head">
          <span class="q-dot" :style="{ background: QUADRANT_META[q].color }"></span>
          <span class="q-name">{{ QUADRANT_META[q].name }}</span>
          <span class="q-count">{{ stats.by_quadrant[q].done }}/{{ stats.by_quadrant[q].total }}</span>
        </div>
        <div class="bar-wrap sm">
          <div
            class="bar-fill"
            :style="{
              width:
                (stats.by_quadrant[q].total
                  ? stats.by_quadrant[q].done / stats.by_quadrant[q].total
                  : 0) * 100 + '%',
              background: QUADRANT_META[q].color,
            }"
          ></div>
        </div>
      </div>

      <!-- Recent activity -->
      <div class="full trend-section anim-slideUp" style="animation-delay: 0.4s;">
        <h3>近 7 天完成趋势</h3>
        <div class="trend">
          <div v-for="d in stats.recent_completed" :key="d.date" class="trend-item">
            <span class="trend-count">{{ d.count || '' }}</span>
            <div class="trend-bar">
              <div
                class="trend-fill"
                :style="{ height: Math.min(100, d.count * 25) + '%' }"
              ></div>
            </div>
            <span class="trend-date">{{ d.date }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.stats-panel {
  padding: 24px 28px;
  overflow-y: auto;
  height: 100%;
}
.stats-error {
  color: var(--danger);
  font-size: 13px;
}
.panel-header {
  margin-bottom: 20px;
}
h2 {
  font-size: 20px;
  font-weight: 700;
}
.sub {
  font-size: 13px;
  color: var(--text-muted);
  margin-top: 4px;
}
h3 {
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 14px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}
.stat-card {
  background: var(--surface);
  border: 1px solid var(--border-light);
  border-radius: var(--radius);
  padding: 20px 16px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: center;
  box-shadow: var(--shadow-sm);
}
.num {
  font-size: 30px;
  font-weight: 700;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}
.lbl {
  font-size: 12px;
  color: var(--text-muted);
}
.full {
  grid-column: span 4;
}
.rate-section {
  padding: 16px;
  background: var(--surface);
  border: 1px solid var(--border-light);
  border-radius: var(--radius);
  box-shadow: var(--shadow-sm);
}
.rate-label {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  font-weight: 500;
  margin-bottom: 10px;
}
.rate-pct {
  color: var(--primary);
  font-weight: 700;
}
.bar-wrap {
  height: 10px;
  background: var(--surface-2);
  border-radius: 5px;
  overflow: hidden;
}
.bar-wrap.sm {
  height: 6px;
  margin-top: 8px;
}
.bar-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--primary), #5b7ff0);
  border-radius: 5px;
  transition: width var(--dur-slow) var(--ease);
}
.q-stat {
  background: var(--surface);
  border: 1px solid var(--border-light);
  border-radius: var(--radius);
  padding: 14px 16px;
  box-shadow: var(--shadow-sm);
}
.q-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.q-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.q-name {
  font-size: 13px;
  font-weight: 500;
  flex: 1;
}
.q-count {
  font-size: 12px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
.trend-section {
  padding: 18px;
  background: var(--surface);
  border: 1px solid var(--border-light);
  border-radius: var(--radius);
  box-shadow: var(--shadow-sm);
}
.trend {
  display: flex;
  align-items: flex-end;
  gap: 12px;
  height: 140px;
  padding: 4px 0;
}
.trend-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  height: 100%;
}
.trend-count {
  font-size: 13px;
  font-weight: 700;
  color: var(--primary);
  min-height: 16px;
  font-variant-numeric: tabular-nums;
}
.trend-bar {
  flex: 1;
  width: 100%;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}
.trend-fill {
  width: 65%;
  background: linear-gradient(180deg, var(--primary), #5b7ff0);
  border-radius: 5px 5px 0 0;
  min-height: 3px;
  transition: height var(--dur-slow) var(--ease);
}
.trend-date {
  font-size: 11px;
  color: var(--text-muted);
}
.loading-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
}
</style>
