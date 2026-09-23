export type Quadrant = 1 | 2 | 3 | 4;

export interface Task {
  id: number;
  title: string;
  description: string;
  quadrant: Quadrant;
  priority: number; // 0-100, higher = more important within quadrant
  importance_score: number; // 0-4 from JevAI
  urgency_score: number; // 0-4 from JevAI
  done: boolean;
  created_at: number; // unix timestamp (ms)
  completed_at: number | null;
}

export interface TaskInput {
  title: string;
  description: string;
  quadrant: Quadrant;
  priority: number;
  importance_score: number;
  urgency_score: number;
}

export interface ClassificationResult {
  quadrant: Quadrant;
  priority: number;
  importance_score: number;
  urgency_score: number;
  importance_label: string;
  urgency_label: string;
}

export interface Settings {
  api_key_configured: boolean;
}

export interface StatsSummary {
  total: number;
  done: number;
  pending: number;
  completion_rate: number;
  by_quadrant: Record<Quadrant, { total: number; done: number }>;
  recent_completed: { date: string; count: number }[];
}

export const QUADRANT_META: Record<
  Quadrant,
  { name: string; subtitle: string; color: string; bg: string; border: string }
> = {
  1: {
    name: "重要且紧急",
    subtitle: "立即做",
    color: "#f53f3f",
    bg: "rgba(245,63,63,0.06)",
    border: "rgba(245,63,63,0.3)",
  },
  2: {
    name: "重要不紧急",
    subtitle: "计划做",
    color: "#3457d5",
    bg: "rgba(52,87,213,0.06)",
    border: "rgba(52,87,213,0.3)",
  },
  3: {
    name: "紧急不重要",
    subtitle: "委托或快做",
    color: "#ff7d00",
    bg: "rgba(255,125,0,0.06)",
    border: "rgba(255,125,0,0.3)",
  },
  4: {
    name: "不重要不紧急",
    subtitle: "删除或少做",
    color: "#86909c",
    bg: "rgba(134,144,156,0.06)",
    border: "rgba(134,144,156,0.3)",
  },
};

export const IMPORTANCE_LABELS = [
  "无关紧要，没有实际影响",
  "影响较小，锦上添花",
  "中等影响，支撑部分目标",
  "重大影响，直接支撑关键目标",
  "至关重要，决定长期成功",
];

export const URGENCY_LABELS = [
  "没有截止时间，随时可做",
  "宽松截止，可等待数周",
  "合理截止，近几天内",
  "紧迫截止，24-48 小时内",
  "必须立刻，已经逾期",
];
