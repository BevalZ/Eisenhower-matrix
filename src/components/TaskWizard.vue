<script setup lang="ts">
import { ref, nextTick, reactive } from "vue";
import { useTasks } from "../composables/useTasks";
import {
  QUADRANT_META,
  scoresForQuadrant,
  type Quadrant,
  type ClassificationResult,
} from "../types";

const emit = defineEmits<{ close: [] }>();
const { classifyTask, createTask, settings, recordQuadrantCorrection } = useTasks();

type Step = "title" | "content" | "importance" | "urgency" | "context" | "confirm";

interface Message {
  role: "ai" | "user";
  text: string;
}

const step = ref<Step>("title");
const messages = ref<Message[]>([
  { role: "ai", text: "你好！我来帮你梳理任务。这个任务叫什么名字？" },
]);
const input = ref("");
const analyzing = ref(false);
const chatBox = ref<HTMLElement>();

const form = reactive({
  title: "",
  content: "",
  whyImportant: "",
  urgency: "",
  context: "",
});

const result = ref<ClassificationResult | null>(null);
const aiClassified = ref(false);
const manualQuadrant = ref<Quadrant>(1);
const manualPriority = ref(50);

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

function nextStep(answer: string) {
  pushMsg("user", answer);

  switch (step.value) {
    case "title":
      form.title = answer;
      step.value = "content";
      pushMsg("ai", "好的。能具体描述一下这个任务要做什么、达到什么结果吗？");
      break;
    case "content":
      form.content = answer;
      step.value = "importance";
      pushMsg("ai", "这件事为什么重要？它和你的什么目标或责任相关？");
      break;
    case "importance":
      form.whyImportant = answer;
      step.value = "urgency";
      pushMsg("ai", "它有截止时间吗？如果拖延会有什么后果？");
      break;
    case "urgency":
      form.urgency = answer;
      step.value = "context";
      pushMsg("ai", "还有其他背景或上下文需要补充吗？没有的话输入「跳过」。");
      break;
    case "context":
      form.context = answer === "跳过" ? "" : answer;
      step.value = "confirm";
      runClassification();
      break;
  }
}

function skip() {
  nextStep("跳过");
}

async function runClassification() {
  analyzing.value = true;
  pushMsg("ai", "正在分析这个任务的位置…");
  try {
    const description = [
      `任务：${form.title}`,
      form.content && `内容：${form.content}`,
      form.whyImportant && `为什么重要：${form.whyImportant}`,
      form.urgency && `紧迫情况：${form.urgency}`,
      form.context && `上下文：${form.context}`,
    ]
      .filter(Boolean)
      .join("\n");

    result.value = await classifyTask(description);
    aiClassified.value = true;
    manualQuadrant.value = result.value.quadrant;
    manualPriority.value = Math.round(result.value.priority);
    pushMsg(
      "ai",
      `分析完成！这个任务属于「${QUADRANT_META[result.value.quadrant].name}」，综合优先级 ${Math.round(result.value.priority)} 分。请确认或调整后保存。`
    );
  } catch (e: any) {
    aiClassified.value = false;
    pushMsg("ai", `AI 分析失败：${e}\n你可以在下方手动选择象限后保存。`);
    result.value = {
      quadrant: 2,
      priority: 50,
      importance_score: 2.5,
      urgency_score: 2.5,
      importance_label: "中等影响，支撑部分目标",
      urgency_label: "合理截止，近几天内",
    };
    manualQuadrant.value = 2;
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
      description: form.content || form.whyImportant || "",
      quadrant: manualQuadrant.value,
      priority,
      importance_score: ai?.importance_score ?? 2.5,
      urgency_score: ai?.urgency_score ?? 2.5,
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
  if (!input.value.trim() || analyzing.value) return;
  const ans = input.value.trim();
  input.value = "";
  nextStep(ans);
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
        <button class="close-btn" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M3 3l8 8M11 3l-8 8"/>
          </svg>
        </button>
      </div>

      <!-- No API key warning -->
      <div v-if="!settings.api_key_configured" class="key-warn">
        ⚠ 尚未配置 API Key，AI 分析不可用。<a href="javascript:void(0)" @click.prevent="emit('close')">去设置</a>
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

      <!-- Input -->
      <div v-if="step !== 'confirm'" class="input-row">
        <input
          v-model="input"
          type="text"
          :placeholder="step === 'context' ? '输入「跳过」可跳过此步…' : '输入你的回答…'"
          @keyup.enter="submit"
        />
        <button v-if="step === 'context'" class="btn btn-ghost" @click="skip">跳过</button>
        <button class="btn btn-primary" :disabled="analyzing" @click="submit">
          发送
        </button>
      </div>

      <!-- Confirm panel -->
      <div v-else class="confirm anim-slideUp">
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
            <option :value="1">{{ QUADRANT_META[1].name }}（{{ QUADRANT_META[1].subtitle }}）</option>
            <option :value="2">{{ QUADRANT_META[2].name }}（{{ QUADRANT_META[2].subtitle }}）</option>
            <option :value="3">{{ QUADRANT_META[3].name }}（{{ QUADRANT_META[3].subtitle }}）</option>
            <option :value="4">{{ QUADRANT_META[4].name }}（{{ QUADRANT_META[4].subtitle }}）</option>
          </select>
          <label>优先级（0-100）</label>
          <input type="text" v-model.number="manualPriority" />
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
  overflow: hidden;
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
.key-warn a {
  color: var(--warning);
  font-weight: 600;
  text-decoration: underline;
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
.input-row {
  display: flex;
  gap: 8px;
  padding: 14px 20px;
  border-top: 1px solid var(--border-light);
  background: var(--surface-2);
}
.confirm {
  padding: 16px 20px;
  border-top: 1px solid var(--border-light);
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
