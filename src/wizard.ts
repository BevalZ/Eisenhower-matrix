import { calendarDaysBetween, type QuickDue } from "./due";
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
  dueAt: number | null;
  consequence: Consequence;
}

const IMPACT_SCORE: Record<Impact, number> = { core: 3.5, helpful: 2.6, minor: 1, unsure: 2.5 };
const CONSEQUENCE_BONUS: Record<Consequence, number> = { severe: 0.6, some: 0, none: -0.6, unsure: 0 };

/** Urgency from how many calendar days are left: today 3.8 … over a month away 0.6. */
function dueScore(dueAt: number | null, now: number): number {
  if (dueAt == null) return 0.6;
  const days = calendarDaysBetween(new Date(now), new Date(dueAt));
  if (days <= 0) return 3.8;
  if (days === 1) return 3.2;
  if (days <= 7) return 2.2;
  if (days <= 14) return 1.6;
  if (days <= 31) return 1;
  return 0.6;
}

/**
 * Offline guess from the answers, used when AI is unavailable. Typed (custom)
 * impact or consequence answers count as "unsure".
 */
export function estimateQuadrant(a: Answers, now: number): Quadrant {
  const urgency = Math.min(4, Math.max(0, dueScore(a.dueAt, now) + CONSEQUENCE_BONUS[a.consequence]));
  const importance = Math.min(4, IMPACT_SCORE[a.impact] + (a.consequence === "severe" ? 0.3 : 0));
  return quadrantFromScores(importance, urgency);
}

export function labelOf<T>(choices: Choice<T>[], value: T): string {
  return choices.find((c) => c.value === value)?.label ?? "";
}

/** The five questions of the new-task wizard, in order. */
export type WizardField = "title" | "impact" | "due" | "consequence" | "note";
export const WIZARD_FIELDS: WizardField[] = ["title", "impact", "due", "consequence", "note"];

export const NOTE_CHOICES: Choice<string>[] = [
  { value: "有人在等结果", label: "有人在等结果" },
  { value: "需要别人配合", label: "需要别人配合" },
  { value: "可以拆成小步做", label: "可以拆成小步做" },
];

/** Tappable answers that ship with the app; titles have none. */
export const BUILT_IN_OPTIONS: Record<WizardField, Choice<string>[]> = {
  title: [],
  impact: IMPACT_CHOICES,
  due: DUE_CHOICES,
  consequence: CONSEQUENCE_CHOICES,
  note: NOTE_CHOICES,
};

/** User-added options, saved per field for later wizards. */
export type CustomOptions = Record<WizardField, string[]>;

const CUSTOM_KEY = "wizardCustomOptions";
export const MAX_CUSTOM_OPTIONS = 12;
const MAX_OPTION_LENGTH = 100; // same as the wizard's input maxlength

export function emptyCustomOptions(): CustomOptions {
  return { title: [], impact: [], due: [], consequence: [], note: [] };
}

/**
 * Keeps only what `addOption` would accept, so a corrupt or hand-edited entry can't
 * break the wizard (e.g. duplicate chips, or one that is now built in).
 */
export function parseCustomOptions(raw: string | null): CustomOptions {
  const out = emptyCustomOptions();
  try {
    const data = JSON.parse(raw ?? "{}");
    for (const f of WIZARD_FIELDS) {
      if (!Array.isArray(data?.[f])) continue;
      const builtIn = BUILT_IN_OPTIONS[f].map((c) => c.label);
      for (const v of data[f]) {
        if (typeof v === "string") out[f] = addOption(out[f], v, builtIn);
      }
    }
  } catch {
    // fall through with empty lists
  }
  return out;
}

export function loadCustomOptions(): CustomOptions {
  try {
    return parseCustomOptions(localStorage.getItem(CUSTOM_KEY));
  } catch {
    return emptyCustomOptions();
  }
}

export function saveCustomOptions(opts: CustomOptions) {
  try {
    localStorage.setItem(CUSTOM_KEY, JSON.stringify(opts));
  } catch {
    // storage full or unavailable: options just won't persist
  }
}

/**
 * Adds `label` to the end of `list` unless it's blank, a duplicate, or already a
 * built-in option. When full, the oldest custom option makes room.
 */
export function addOption(list: string[], label: string, builtIn: string[] = []): string[] {
  const text = label.trim().slice(0, MAX_OPTION_LENGTH);
  if (!text || list.includes(text) || builtIn.includes(text)) return list;
  return [...list, text].slice(-MAX_CUSTOM_OPTIONS);
}

export function removeOption(list: string[], label: string): string[] {
  return list.filter((v) => v !== label);
}
