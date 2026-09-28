import type { QuickDue } from "./due";
import { quadrantFromScores, type Quadrant } from "./types";

export interface Choice<T> {
  value: T;
  label: string;
}

export type Impact = "core" | "helpful" | "minor" | "unsure";
export type Consequence = "severe" | "some" | "none" | "unsure";

export const IMPACT_CHOICES: Choice<Impact>[] = [
  { value: "core", label: "关乎核心目标或职责" },
  { value: "helpful", label: "有帮助，但不关键" },
  { value: "minor", label: "可有可无" },
  { value: "unsure", label: "说不清" },
];

export const DUE_CHOICES: Choice<QuickDue>[] = [
  { value: "today", label: "今天" },
  { value: "tomorrow", label: "明天" },
  { value: "week", label: "本周内" },
  { value: "nextWeek", label: "下周" },
  { value: "month", label: "一个月内" },
  { value: "none", label: "没有截止" },
];

export const CONSEQUENCE_CHOICES: Choice<Consequence>[] = [
  { value: "severe", label: "后果严重" },
  { value: "some", label: "有点麻烦" },
  { value: "none", label: "基本没影响" },
  { value: "unsure", label: "说不清" },
];

export interface Answers {
  impact: Impact;
  due: QuickDue;
  consequence: Consequence;
}

const IMPACT_SCORE: Record<Impact, number> = { core: 3.5, helpful: 2.6, minor: 1, unsure: 2.5 };
const DUE_SCORE: Record<QuickDue, number> = {
  today: 3.8, tomorrow: 3.2, week: 2.2, nextWeek: 1.6, month: 1, none: 0.6,
};
const CONSEQUENCE_BONUS: Record<Consequence, number> = { severe: 0.6, some: 0, none: -0.6, unsure: 0 };

/** Offline guess from the tapped answers, used when AI is unavailable. */
export function estimateQuadrant(a: Answers): Quadrant {
  const urgency = Math.min(4, Math.max(0, DUE_SCORE[a.due] + CONSEQUENCE_BONUS[a.consequence]));
  const importance = Math.min(4, IMPACT_SCORE[a.impact] + (a.consequence === "severe" ? 0.3 : 0));
  return quadrantFromScores(importance, urgency);
}

export function labelOf<T>(choices: Choice<T>[], value: T): string {
  return choices.find((c) => c.value === value)?.label ?? "";
}
