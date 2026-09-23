<script setup lang="ts">
import { ref, nextTick, reactive } from "vue";
import { useTasks } from "../composables/useTasks";
import {
  QUADRANT_META,
  IMPORTANCE_LABELS,
  URGENCY_LABELS,
  type Quadrant,
  type ClassificationResult,
} from "../types";

const emit = defineEmits<{ close: [] }>();
const { classifyTask, createTask, settings } = useTasks();

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
const manualQuadrant = ref<Quadrant>(1);
const manualPriority = ref(50);

async function scrollBottom() {
  await nextTick();
  if (chatBox.value) chatBox.value.scrollTop = chatBox.value.scrollHeight;
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
  pushMsg("ai", "正在用 AI 分析这个任务的位置，请稍候…");
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
    manualQuadrant.value = result.value.quadrant;
    manualPriority.value = Math.round(result.value.priority);
    pushMsg(
      "ai",
      `分析完成！这个任务属于「${QUADRANT_META[result.value.quadrant].name}」，综合优先级 ${Math.round(result.value.priority)} 分。请确认或调整后保存。`
    );
  } catch (e: any) {
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
  await createTask({
    title: form.title,
    description: form.content || form.whyImportant || "",
    quadrant: manualQuadrant.value,
    priority: manualPriority.value,
    importance_score: result.value?.importance_score ?? 2.5,
    urgency_score: result.value?.urgency_score ?? 2.5,
  });
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
    <div class="wizard">
      <div class="wizard-head">
        <h3>新建任务</h3>
        <button class="close-btn" @click="emit('close')">✕</button>
      </div>

      <!-- Chat -->
      <div ref="chatBox" class="chat">
        <div v-for="(m, i) in messages" :key="i" class="msg" :class="m.role">
          <div class="bubble">{{ m.text }}</div>
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
        <button class="btn btn-primary" :disabled="analyzing" @click="submit">发送</button>
      </div>

      <!-- Confirm panel -->
      <div v-else class="confirm">
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
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.wizard {
  width: 480px;
  max-height: 85vh;
  background: var(--surface);
  border-radius: 14px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.2);
}
.wizard-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 18px;
  border-bottom: 1px solid var(--border);
}
.close-btn {
  font-size: 16px;
  color: var(--text-muted);
}
.chat {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 200px;
}
.msg {
  display: flex;
}
.msg.user {
  justify-content: flex-end;
}
.bubble {
  max-width: 80%;
  padding: 9px 13px;
  border-radius: 12px;
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-wrap;
}
.msg.ai .bubble {
  background: var(--bg);
  border-bottom-left-radius: 4px;
}
.msg.user .bubble {
  background: var(--primary);
  color: #fff;
  border-bottom-right-radius: 4px;
}
.input-row {
  display: flex;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border);
}
.confirm {
  padding: 16px;
  border-top: 1px solid var(--border);
}
.result-cards {
  display: flex;
  gap: 12px;
  margin-bottom: 14px;
}
.r-card {
  flex: 1;
  padding: 10px;
  background: var(--bg);
  border-radius: 8px;
}
.r-card label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
}
.score-bar {
  height: 6px;
  background: var(--border);
  border-radius: 3px;
  margin: 6px 0;
  overflow: hidden;
}
.fill {
  height: 100%;
  border-radius: 3px;
}
.fill.imp {
  background: #3182ce;
}
.fill.urg {
  background: #d69e2e;
}
.r-label {
  font-size: 11px;
  color: var(--text-muted);
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
  color: var(--text-muted);
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
