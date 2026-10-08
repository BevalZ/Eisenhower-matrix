<script setup lang="ts">
import { ref, nextTick, reactive, computed } from "vue";
import { useTasks } from "../composables/useTasks";
import {
  QUADRANT_META,
  DEFAULT_PRIORITY,
  UNSCORED,
  scoresForQuadrant,
  type Quadrant,
  type ClassificationResult,
} from "../types";
import { fromLocalInput, quickDue, toLocalInput, dueLabel, parseDueText, type QuickDue } from "../due";
import { errorText } from "../composables/useToast";
import {
  IMPACT_CHOICES,
  DUE_CHOICES,
  CONSEQUENCE_CHOICES,
  BUILT_IN_OPTIONS,
  estimateQuadrant,
  labelOf,
  loadCustomOptions,
  saveCustomOptions,
  addOption,
  removeOption,
  type CustomOptions,
  type Impact,
  type Consequence,
  type WizardField,
} from "../wizard";

const emit = defineEmits<{ close: []; "open-settings": [] }>();
const { classifyTask, createTask, settings, recordQuadrantCorrection } = useTasks();

type Step = WizardField | "confirm";

interface Message {
  role: "ai" | "user";
  text: string;
}

/** A tappable answer; `value` is set only for built-in options. */
interface Option {
  label: string;
  value?: string;
}

const step = ref<Step>("title");
const messages = ref<Message[]>([
  { role: "ai", text: "你好！用一句话写下要做的事吧。" },
]);
const input = ref("");
const saveOption = ref(false);
const inputError = ref("");
const analyzing = ref(false);
const chatBox = ref<HTMLElement>();
const custom = ref<CustomOptions>(loadCustomOptions());

const form = reactive({
  title: "",
  impact: "unsure" as Impact,
  impactLabel: labelOf(IMPACT_CHOICES, "unsure"),
  dueLabel: labelOf(DUE_CHOICES, "none"),
  consequence: "unsure" as Consequence,
  consequenceLabel: labelOf(CONSEQUENCE_CHOICES, "unsure"),
  note: "",
});

const result = ref<ClassificationResult | null>(null);
const aiClassified = ref(false);
const manualQuadrant = ref<Quadrant>(1);
const manualPriority = ref(50);
const dueInput = ref("");

const QUESTIONS: Partial<Record<Step, string>> = {
  impact: "做好这件事，影响有多大？",
  due: "什么时候要完成？",
  consequence: "如果拖着不做，会怎样？",
  note: "还想补充一句吗？（可选，比如谁在等结果）",
};

const PLACEHOLDERS: Record<WizardField, string> = {
  title: "例如：周五前交季度报告",
  impact: "或用自己的话说，如：影响客户续约",
  due: "或输入时间，如：3天内、周五、10月5日",
  consequence: "或用自己的话说，如：会被扣绩效",
  note: "一句话即可，没有就跳过",
};

const NEXT: Record<WizardField, Step> = {
  title: "impact",
  impact: "due",
  due: "consequence",
  consequence: "note",
  note: "confirm",
};

const field = computed<WizardField | null>(() => (step.value === "confirm" ? null : step.value));

const options = computed<Option[]>(() => {
  const f = field.value;
  if (!f) return [];
  return [...BUILT_IN_OPTIONS[f], ...custom.value[f].map((label) => ({ label }))];
});

async function scrollBottom() {
  await nextTick();
  if (chatBox.value) chatBox.value.scrollTo({
    top: chatBox.value.scrollHeight,
    behavior: "smooth",
  });
}

function pushMsg(role: "ai" | "user", text: string) {
  messages.value.push({ role, text });
  scrollBottom();
}

function goTo(next: Step) {
  step.value = next;
  input.value = "";
  saveOption.value = false;
  inputError.value = "";
  const q = QUESTIONS[next];
  if (q) pushMsg("ai", q);
  if (next === "confirm") runClassification();
}

/**
 * Records one answer and moves on. Typed text that matches a built-in label counts
 * as tapping it. Returns false (and stays on the step) if a typed deadline isn't understood.
 */
function answer(f: WizardField, text: string, value?: string): boolean {
  value ??= BUILT_IN_OPTIONS[f].find((c) => c.label === text)?.value;
  switch (f) {
    case "title":
      form.title = text;
      break;
    case "impact":
      form.impact = (value ?? "unsure") as Impact;
      form.impactLabel = text;
      break;
    case "due": {
      const now = Date.now();
      const due = value !== undefined ? quickDue(value as QuickDue, now) : parseDueText(text, now);
      if (due === undefined) {
        inputError.value = "没看懂这个时间，可以写成「3天内」「周五」「下周三」「10月5日」";
        return false;
      }
      form.dueLabel = text;
      dueInput.value = toLocalInput(due);
      break;
    }
    case "consequence":
      form.consequence = (value ?? "unsure") as Consequence;
      form.consequenceLabel = text;
      break;
    case "note":
      form.note = text;
      break;
  }
  pushMsg("user", text);
  goTo(NEXT[f]);
  return true;
}

function pick(opt: Option) {
  if (analyzing.value || !field.value) return;
  answer(field.value, opt.label, opt.value);
}

function remember(f: WizardField, text: string) {
  const list = addOption(custom.value[f], text, BUILT_IN_OPTIONS[f].map((c) => c.label));
  if (list === custom.value[f]) return;
  custom.value = { ...custom.value, [f]: list };
  saveCustomOptions(custom.value);
}

function forget(f: WizardField, label: string) {
  custom.value = { ...custom.value, [f]: removeOption(custom.value[f], label) };
  saveCustomOptions(custom.value);
}

function skip() {
  pushMsg("user", "跳过");
  goTo("confirm");
}

function describe(): string {
  const due = fromLocalInput(dueInput.value);
  return [
    `任务：${form.title}`,
    `影响：${form.impactLabel}`,
    `截止：${due == null ? "没有截止" : `${form.dueLabel}（${dueLabel(due, Date.now())}）`}`,
    `拖延后果：${form.consequenceLabel}`,
    form.note && `补充：${form.note}`,
  ]
    .filter(Boolean)
    .join("\n");
}

async function runClassification() {
  analyzing.value = true;
  pushMsg("ai", "正在分析这个任务的位置…");
  try {
    result.value = await classifyTask(describe());
    aiClassified.value = true;
    manualQuadrant.value = result.value.quadrant;
    manualPriority.value = Math.round(result.value.priority);
    pushMsg(
      "ai",
      `分析完成！这个任务属于「${QUADRANT_META[result.value.quadrant].name}」，综合优先级 ${Math.round(result.value.priority)} 分。请确认或调整后保存。`
    );
  } catch (e) {
    // Fall back to a guess from the tapped answers; scores stay UNSCORED so a
    // later move isn't learned as a correction of the AI.
    aiClassified.value = false;
    const quadrant = estimateQuadrant(
      { impact: form.impact, consequence: form.consequence, dueAt: fromLocalInput(dueInput.value) },
      Date.now()
    );
    result.value = null;
    manualQuadrant.value = quadrant;
    manualPriority.value = DEFAULT_PRIORITY[quadrant];
    pushMsg(
      "ai",
      `AI 分析不可用（${errorText(e)}）。\n根据你的选择，建议放在「${QUADRANT_META[quadrant].name}」，可在下方调整后保存。`
    );
  } finally {
    analyzing.value = false;
  }
}

async function save() {
  const priority = Number.isFinite(manualPriority.value)
    ? Math.min(100, Math.max(0, manualPriority.value))
    : 50;
  const ai = result.value;
  try {
    await createTask({
      title: form.title,
      description: form.note,
      quadrant: manualQuadrant.value,
      priority,
      importance_score: ai?.importance_score ?? UNSCORED,
      urgency_score: ai?.urgency_score ?? UNSCORED,
      due_at: fromLocalInput(dueInput.value),
    });
  } catch (err) {
    pushMsg("ai", `保存失败：${err}`);
    return;
  }
  if (aiClassified.value && ai && manualQuadrant.value !== ai.quadrant) {
    const user = scoresForQuadrant(ai.importance_score, ai.urgency_score, manualQuadrant.value);
    recordQuadrantCorrection({
      taskTitle: form.title,
      aiImportance: ai.importance_score,
      aiUrgency: ai.urgency_score,
      aiQuadrant: ai.quadrant,
      userImportance: user.importance,
      userUrgency: user.urgency,
      userQuadrant: manualQuadrant.value,
    });
  }
  emit("close");
}

function submit() {
  const f = field.value;
  const text = input.value.trim();
  if (!f || !text || analyzing.value) return;
  const save = saveOption.value; // answer() moves on and resets the checkbox
  if (answer(f, text) && save) remember(f, text);
}
</script>

<template>
  <div class="wizard-overlay" @click.self="emit('close')">
    <div class="wizard anim-popIn">
      <div class="wizard-head">
        <div class="head-left">
          <div class="ai-avatar">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <rect x="1" y="1" width="6" height="6" rx="1.5" fill="var(--q1)" opacity="0.8"/>
              <rect x="9" y="1" width="6" height="6" rx="1.5" fill="var(--q2)" opacity="0.8"/>
              <rect x="1" y="9" width="6" height="6" rx="1.5" fill="var(--q3)" opacity="0.8"/>
              <rect x="9" y="9" width="6" height="6" rx="1.5" fill="var(--q4)" opacity="0.8"/>
            </svg>
          </div>
          <div>
            <h3>新建任务</h3>
            <span class="head-sub">AI 对话式创建</span>
          </div>
        </div>
        <button class="close-btn" aria-label="关闭" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M3 3l8 8M11 3l-8 8"/>
          </svg>
        </button>
      </div>

      <!-- No API key warning -->
      <div v-if="!settings.api_key_configured" class="key-warn">
        ⚠ 尚未配置 API Key，AI 分析不可用。<button class="link" @click="emit('open-settings')">去设置</button>
      </div>

      <!-- Chat -->
      <div ref="chatBox" class="chat">
        <div v-for="(m, i) in messages" :key="i" class="msg" :class="m.role">
          <div v-if="m.role === 'ai'" class="msg-avatar">AI</div>
          <div class="bubble">{{ m.text }}</div>
        </div>
        <div v-if="analyzing" class="msg ai">
          <div class="msg-avatar">AI</div>
          <div class="bubble typing">
            <span></span><span></span><span></span>
          </div>
        </div>
      </div>

      <!-- Answer: tap an option, or type one (and optionally keep it for next time) -->
      <form v-if="field" class="answer" @submit.prevent="submit">
        <div v-if="options.length" class="choice-row">
          <template v-for="o in options" :key="o.label">
            <button v-if="o.value" type="button" class="chip" @click="pick(o)">{{ o.label }}</button>
            <span v-else class="chip custom">
              <button type="button" class="chip-pick" :title="o.label" @click="pick(o)">{{ o.label }}</button>
              <button
                type="button"
                class="chip-del"
                :aria-label="`删除选项「${o.label}」`"
                title="删除这个选项"
                @click="forget(field, o.label)"
              >
                <svg width="8" height="8" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                  <path d="M3 3l8 8M11 3l-8 8"/>
                </svg>
              </button>
            </span>
          </template>
        </div>
        <div class="input-row">
          <input
            v-model="input"
            type="text"
            maxlength="100"
            :placeholder="PLACEHOLDERS[field]"
            autofocus
            @input="inputError = ''"
            @focus="scrollBottom"
          />
          <button v-if="step === 'note'" type="button" class="btn btn-ghost" @click="skip">跳过</button>
          <button type="submit" class="btn btn-primary" :disabled="!input.trim()">
            {{ step === 'title' ? '下一步' : '发送' }}
          </button>
        </div>
        <p v-if="inputError" class="input-error" role="alert">{{ inputError }}</p>
        <label class="save-opt" title="下次新建任务时可以直接点选">
          <input v-model="saveOption" type="checkbox" />保存为选项
        </label>
      </form>

      <!-- Confirm panel -->
      <div v-if="step === 'confirm' && !analyzing" class="confirm anim-slideUp">
        <div v-if="result" class="result-cards">
          <div class="r-card">
            <label>重要性</label>
            <div class="score-bar">
              <div class="fill imp" :style="{ width: (result.importance_score / 4) * 100 + '%' }"></div>
            </div>
            <span class="r-label">{{ result.importance_label }}</span>
          </div>
          <div class="r-card">
            <label>紧急性</label>
            <div class="score-bar">
              <div class="fill urg" :style="{ width: (result.urgency_score / 4) * 100 + '%' }"></div>
            </div>
            <span class="r-label">{{ result.urgency_label }}</span>
          </div>
        </div>

        <div class="adjust">
          <label>象限</label>
          <select v-model.number="manualQuadrant">
            <option v-for="q in ([1, 2, 3, 4] as const)" :key="q" :value="q">
              {{ QUADRANT_META[q].name }}（{{ QUADRANT_META[q].subtitle }}）
            </option>
          </select>
          <label>优先级：{{ manualPriority }}</label>
          <input type="range" min="0" max="100" step="1" v-model.number="manualPriority" />
          <label>截止时间</label>
          <input type="datetime-local" v-model="dueInput" />
        </div>

        <div class="actions">
          <button class="btn btn-ghost" @click="emit('close')">取消</button>
          <button class="btn btn-primary" @click="save">保存任务</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wizard-overlay {
  position: fixed;
  inset: 0;
  background: rgba(29, 33, 41, 0.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.wizard {
  width: 500px;
  max-height: 86vh;
  background: var(--surface);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  box-shadow: var(--shadow-lg);
}
.wizard-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-light);
}
.head-left {
  display: flex;
  align-items: center;
  gap: 10px;
}
.ai-avatar {
  width: 32px;
  height: 32px;
  background: var(--surface-2);
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
}
h3 {
  font-size: 15px;
  font-weight: 600;
  line-height: 1.2;
}
.head-sub {
  font-size: 11px;
  color: var(--text-muted);
}
.close-btn {
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
.key-warn {
  padding: 8px 20px;
  background: var(--warning-light);
  color: var(--warning);
  font-size: 12px;
}
.key-warn .link {
  color: var(--warning);
  font-weight: 600;
  text-decoration: underline;
  padding: 0;
}
.chat {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 240px;
}
.msg {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  animation: slideUp var(--dur) var(--ease);
}
.msg.user {
  justify-content: flex-end;
}
.msg-avatar {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  background: var(--primary);
  color: #fff;
  font-size: 10px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.bubble {
  max-width: 78%;
  padding: 10px 14px;
  border-radius: 12px;
  font-size: 13px;
  line-height: 1.55;
  white-space: pre-wrap;
}
.msg.ai .bubble {
  background: var(--surface-2);
  border-bottom-left-radius: 4px;
  color: var(--text);
}
.msg.user .bubble {
  background: var(--primary);
  color: #fff;
  border-bottom-right-radius: 4px;
}
.typing {
  display: flex;
  gap: 4px;
  align-items: center;
  padding: 12px 16px;
}
.typing span {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-muted);
  animation: typingBounce 1.4s infinite ease-in-out;
}
.typing span:nth-child(2) { animation-delay: 0.2s; }
.typing span:nth-child(3) { animation-delay: 0.4s; }
@keyframes typingBounce {
  0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
  30% { transform: translateY(-4px); opacity: 1; }
}
.answer {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 20px;
  border-top: 1px solid var(--border-light);
  background: var(--surface-2);
}
.input-row {
  display: flex;
  gap: 8px;
}
.confirm {
  padding: 16px 20px;
  border-top: 1px solid var(--border-light);
}
.choice-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  max-height: 124px; /* about three rows; saved options scroll */
  overflow-y: auto;
}
.chip {
  max-width: 100%;
  padding: 7px 14px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: border-color var(--dur) var(--ease), color var(--dur) var(--ease);
}
.chip:hover {
  border-color: var(--primary);
  color: var(--primary);
}
/* A saved option: the label picks it, × removes it */
.chip.custom {
  display: inline-flex;
  align-items: center;
  padding: 0 5px 0 0;
}
.chip-pick {
  min-width: 0;
  padding: 7px 6px 7px 14px;
  color: inherit;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chip-del {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  color: var(--text-muted);
}
.chip-del:hover {
  background: var(--danger-light);
  color: var(--danger);
}
.input-error {
  font-size: 12px;
  color: var(--danger);
}
.save-opt {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  align-self: flex-start;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
}
.save-opt input {
  accent-color: var(--primary);
}
.result-cards {
  display: flex;
  gap: 12px;
  margin-bottom: 14px;
}
.r-card {
  flex: 1;
  padding: 12px;
  background: var(--surface-2);
  border-radius: var(--radius);
}
.r-card label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
}
.score-bar {
  height: 6px;
  background: var(--border);
  border-radius: 3px;
  margin: 8px 0;
  overflow: hidden;
}
.fill {
  height: 100%;
  border-radius: 3px;
  transition: width var(--dur-slow) var(--ease);
}
.fill.imp {
  background: var(--q2);
}
.fill.urg {
  background: var(--q3);
}
.r-label {
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.4;
}
.adjust {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 14px;
}
.adjust label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
