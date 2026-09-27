<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { QUADRANT_META, type Quadrant, type Task } from "../types";
import { fromLocalInput, toLocalInput } from "../due";
import { useTasks } from "../composables/useTasks";

const props = defineProps<{ task: Task }>();
const emit = defineEmits<{ close: [] }>();
const { updateTask } = useTasks();

const form = reactive({
  title: props.task.title,
  description: props.task.description,
  quadrant: props.task.quadrant as Quadrant,
  priority: Math.round(props.task.priority * 10) / 10,
  due: toLocalInput(props.task.due_at),
});
const error = ref("");
const saving = ref(false);
const titleInput = ref<HTMLInputElement>();
// Same 2×2 arrangement as the board: important on top, urgent on the right.
const pickerOrder: Quadrant[] = [2, 1, 4, 3];

onMounted(() => titleInput.value?.focus());

async function save() {
  if (saving.value) return;
  const title = form.title.trim();
  const priority = Number(form.priority);
  if (!title) {
    error.value = "标题不能为空";
    return;
  }
  if (!Number.isFinite(priority) || priority < 0 || priority > 100) {
    error.value = "优先级需要是 0 到 100 之间的数字";
    return;
  }
  saving.value = true;
  error.value = "";
  try {
    await updateTask({
      id: props.task.id,
      title,
      description: form.description.trim(),
      quadrant: form.quadrant,
      priority,
      due_at: fromLocalInput(form.due),
    });
    emit("close");
  } catch (err) {
    error.value = `保存失败：${err}`;
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')" @keydown.esc.stop="emit('close')">
    <form class="dialog anim-popIn" role="dialog" aria-modal="true" aria-labelledby="edit-title" @submit.prevent="save">
      <div class="head">
        <h3 id="edit-title">编辑任务</h3>
        <button type="button" class="close-btn" aria-label="关闭" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M3 3l8 8M11 3l-8 8"/></svg>
        </button>
      </div>
      <label class="field">
        <span>标题</span>
        <input ref="titleInput" v-model="form.title" type="text" maxlength="2000" />
      </label>
      <label class="field">
        <span>描述</span>
        <textarea v-model="form.description" rows="3" maxlength="20000"></textarea>
      </label>
      <fieldset class="field">
        <legend>象限</legend>
        <div class="picker">
          <label
            v-for="q in pickerOrder"
            :key="q"
            class="pick"
            :class="{ active: form.quadrant === q }"
            :style="{ '--q-color': QUADRANT_META[q].color, '--q-light': QUADRANT_META[q].bg }"
          >
            <input v-model="form.quadrant" type="radio" name="quadrant" :value="q" class="sr-only" />
            <span class="pick-name">{{ QUADRANT_META[q].name }}</span>
            <span class="pick-sub">{{ QUADRANT_META[q].subtitle }}</span>
          </label>
        </div>
      </fieldset>
      <label class="field">
        <span>优先级（0-100，决定象限内的顺序）</span>
        <input v-model.number="form.priority" type="number" min="0" max="100" step="1" />
      </label>
      <label class="field">
        <span>截止时间（可选，临近时会提醒并建议移到紧急象限）</span>
        <div class="due-row">
          <input v-model="form.due" type="datetime-local" />
          <button v-if="form.due" type="button" class="btn btn-ghost btn-sm" @click="form.due = ''">清除</button>
        </div>
      </label>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <div class="actions">
        <button type="button" class="btn btn-ghost" @click="emit('close')">取消</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">{{ saving ? "保存中…" : "保存" }}</button>
      </div>
    </form>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: 460px;
  max-height: 86vh;
  overflow-y: auto;
  background: var(--surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  padding: 18px 22px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.head { display: flex; align-items: center; justify-content: space-between; }
h3 { font-size: 16px; font-weight: 700; }
.close-btn {
  display: flex; align-items: center; justify-content: center;
  width: 28px; height: 28px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
}
.close-btn:hover { background: var(--surface-2); color: var(--text); }
.field { display: flex; flex-direction: column; gap: 6px; border: none; }
.field > span, legend { font-size: 12px; font-weight: 500; color: var(--text-secondary); margin-bottom: 0; }
legend { margin-bottom: 6px; }
.picker { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.pick {
  display: flex; flex-direction: column; gap: 2px;
  padding: 8px 10px;
  border: 1.5px solid var(--border);
  border-radius: var(--radius);
  cursor: pointer;
  transition: border-color var(--dur-fast) var(--ease), background var(--dur-fast) var(--ease);
}
.pick:hover { border-color: var(--q-color); }
.pick.active { border-color: var(--q-color); background: var(--q-light); }
.pick:focus-within { outline: 2px solid var(--q-color); outline-offset: 1px; }
.pick-name { font-size: 13px; font-weight: 600; color: var(--q-color); }
.pick-sub { font-size: 11px; color: var(--text-muted); }
.error { font-size: 12px; color: var(--danger); }
.due-row { display: flex; gap: 8px; align-items: center; }

.actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
</style>