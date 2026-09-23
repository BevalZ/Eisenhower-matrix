<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useTasks } from "../composables/useTasks";
import { QUADRANT_META, type StatsSummary, type Quadrant } from "../types";

const { getStats } = useTasks();
const stats = ref<StatsSummary | null>(null);

onMounted(async () => {
  stats.value = await getStats();
});

const quadrants: Quadrant[] = [1, 2, 3, 4];
</script>

<template>
  <div class="stats-panel">
    <h2>统计概览</h2>
    <div v-if="stats" class="grid">
      <!-- Overall cards -->
      <div class="stat-card">
        <span class="num">{{ stats.total }}</span>
        <span class="lbl">总任务</span>
      </div>
      <div class="stat-card">
        <span class="num" style="color: var(--success)">{{ stats.done }}</span>
        <span class="lbl">已完成</span>
      </div>
      <div class="stat-card">
        <span class="num" style="color: var(--danger)">{{ stats.pending }}</span>
        <span class="lbl">待处理</span>
      </div>
      <div class="stat-card">
        <span class="num">{{ Math.round(stats.completion_rate * 100) }}%</span>
        <span class="lbl">完成率</span>
      </div>

      <!-- Completion rate bar -->
      <div class="full">
        <div class="bar-wrap">
          <div class="bar-fill" :style="{ width: stats.completion_rate * 100 + '%' }"></div>
        </div>
      </div>

      <!-- Per-quadrant -->
      <div v-for="q in quadrants" :key="q" class="q-stat">
        <div class="q-head">
          <span class="q-dot" :style="{ background: QUADRANT_META[q].color }"></span>
          <span class="q-name">{{ QUADRANT_META[q].name }}</span>
        </div>
        <div class="q-nums">
          <span>共 {{ stats.by_quadrant[q].total }}</span>
          <span>已完成 {{ stats.by_quadrant[q].done }}</span>
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
      <div class="full">
        <h3>近 7 天完成趋势</h3>
        <div class="trend">
          <div v-for="d in stats.recent_completed" :key="d.date" class="trend-item">
            <span class="trend-count">{{ d.count }}</span>
            <div class="trend-bar">
              <div
                class="trend-fill"
                :style="{ height: Math.min(100, d.count * 20) + '%' }"
              ></div>
            </div>
            <span class="trend-date">{{ d.date.slice(5) }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.stats-panel {
  padding: 24px;
  overflow-y: auto;
  height: 100%;
}
h2 {
  margin-bottom: 18px;
  font-size: 18px;
}
h3 {
  font-size: 14px;
  margin-bottom: 10px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}
.stat-card {
  background: var(--surface);
  border-radius: var(--radius);
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  align-items: center;
  box-shadow: var(--shadow);
}
.num {
  font-size: 28px;
  font-weight: 700;
}
.lbl {
  font-size: 12px;
  color: var(--text-muted);
}
.full {
  grid-column: span 4;
}
.bar-wrap {
  height: 10px;
  background: var(--border);
  border-radius: 5px;
  overflow: hidden;
  margin-top: 4px;
}
.bar-wrap.sm {
  height: 6px;
  margin-top: 6px;
}
.bar-fill {
  height: 100%;
  background: var(--success);
  border-radius: 5px;
  transition: width 0.4s;
}
.q-stat {
  background: var(--surface);
  border-radius: var(--radius);
  padding: 14px;
  box-shadow: var(--shadow);
}
.q-head {
  display: flex;
  align-items: center;
  gap: 6px;
}
.q-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.q-name {
  font-size: 13px;
  font-weight: 500;
}
.q-nums {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--text-muted);
  margin: 6px 0;
}
.trend {
  display: flex;
  align-items: flex-end;
  gap: 10px;
  height: 120px;
  padding: 10px 0;
}
.trend-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  height: 100%;
}
.trend-count {
  font-size: 12px;
  font-weight: 600;
}
.trend-bar {
  flex: 1;
  width: 100%;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}
.trend-fill {
  width: 60%;
  background: var(--primary);
  border-radius: 4px 4px 0 0;
  min-height: 2px;
  transition: height 0.4s;
}
.trend-date {
  font-size: 10px;
  color: var(--text-muted);
}
</style>
