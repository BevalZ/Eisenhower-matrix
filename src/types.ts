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
  due_at: number | null; // deadline, unix ms
}

export interface TaskInput {
  title: string;
  description: string;
  quadrant: Quadrant;
  priority: number;
  importance_score: number;
  urgency_score: number;
  due_at?: number | null;
}

export interface TaskUpdate {
  id: number;
  title: string;
  description: string;
  quadrant: Quadrant;
  priority: number;
  due_at: number | null;
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
  theme: string;
  webdav_configured: boolean;
  ai_provider?: "jevai" | "openai";
  ai_base_url?: string;
  ai_model?: string;
  jevai_key_configured?: boolean;
  openai_key_configured?: boolean;
}

export interface AiConfig {
  provider: "jevai" | "openai";
  base_url: string;
  model: string;
  api_key: string;
}


export interface WebdavConfig {
  url: string;
  username: string;
  password: string;
}

export interface WebdavInfo {
  url: string;
  username: string;
  has_password: boolean;
}

export interface SyncResult {
  success: boolean;
  message: string;
  task_count: number;
}

export interface TailscalePeer {
  hostname: string;
  ip: string;
  online: boolean;
  os: string;
}

export interface TailscaleStatus {
  running: boolean;
  hostname: string;
  ip: string;
  peers: TailscalePeer[];
  message: string;
}

export interface PeerSyncStatus {
  listening: boolean;
  address: string;
  port: number;
  secret: string;
  device_id: string;
  last_error: string;
}

export interface StatsSummary {
  total: number;
  done: number;
  pending: number;
  completion_rate: number;
  by_quadrant: Record<Quadrant, { total: number; done: number }>;
  recent_completed: { date: string; count: number }[];
  week_by_quadrant: Record<Quadrant, number>;
  overdue: number;
}


export interface FeedbackRecord {
  id: number;
  task_title: string;
  ai_importance: number;
  ai_urgency: number;
  ai_quadrant: number;
  user_importance: number;
  user_urgency: number;
  user_quadrant: number;
  created_at: number;
}

export interface LearningStats {
  total_corrections: number;
  importance_bias: number;
  urgency_bias: number;
  accuracy_rate: number;
  recent_corrections: FeedbackRecord[];
}

/** Colors are CSS variables so the dark theme in style.css applies. */
export const QUADRANT_META: Record<
  Quadrant,
  { name: string; subtitle: string; color: string; bg: string; border: string }
> = {
  1: {
    name: "重要且紧急",
    subtitle: "立即做",
    color: "var(--q1)",
    bg: "var(--q1-light)",
    border: "var(--q1-border)",
  },
  2: {
    name: "重要不紧急",
    subtitle: "计划做",
    color: "var(--q2)",
    bg: "var(--q2-light)",
    border: "var(--q2-border)",
  },
  3: {
    name: "紧急不重要",
    subtitle: "委托或快做",
    color: "var(--q3)",
    bg: "var(--q3-light)",
    border: "var(--q3-border)",
  },
  4: {
    name: "不重要不紧急",
    subtitle: "删除或少做",
    color: "var(--q4)",
    bg: "var(--q4-light)",
    border: "var(--q4-border)",
  },
};

const SCORE_THRESHOLD = 2.5;

/**
 * Scores given to tasks the AI never rated (quick add, AI unavailable).
 * Moving such a task must not be learned as a correction of the AI.
 */
export const UNSCORED = 2.5;

export function isUnscored(t: { importance_score: number; urgency_score: number }): boolean {
  return t.importance_score === UNSCORED && t.urgency_score === UNSCORED;
}

/** Starting priority for tasks added without AI. */
export const DEFAULT_PRIORITY: Record<Quadrant, number> = { 1: 70, 2: 60, 3: 40, 4: 20 };

export function quadrantFromScores(importance: number, urgency: number): Quadrant {
  const important = importance >= SCORE_THRESHOLD;
  const urgent = urgency >= SCORE_THRESHOLD;
  if (important && urgent) return 1;
  if (important) return 2;
  if (urgent) return 3;
  return 4;
}

/** Nudge scores across the 2.5 threshold without discarding the original magnitude. */
export function scoresForQuadrant(
  importance: number,
  urgency: number,
  quadrant: Quadrant,
): { importance: number; urgency: number } {
  let nextImportance = importance;
  let nextUrgency = urgency;
  const wantImportant = quadrant <= 2;
  const wantUrgent = quadrant === 1 || quadrant === 3;
  if (wantImportant && nextImportance < SCORE_THRESHOLD) nextImportance = SCORE_THRESHOLD;
  if (!wantImportant && nextImportance >= SCORE_THRESHOLD) nextImportance = SCORE_THRESHOLD - 0.01;
  if (wantUrgent && nextUrgency < SCORE_THRESHOLD) nextUrgency = SCORE_THRESHOLD;
  if (!wantUrgent && nextUrgency >= SCORE_THRESHOLD) nextUrgency = SCORE_THRESHOLD - 0.01;
  return {
    importance: Math.min(4, Math.max(0, nextImportance)),
    urgency: Math.min(4, Math.max(0, nextUrgency)),
  };
}
