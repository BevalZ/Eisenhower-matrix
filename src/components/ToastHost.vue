<script setup lang="ts">
import { useToast, type Toast } from "../composables/useToast";

const { toasts, dismiss } = useToast();

async function runAction(t: Toast) {
  dismiss(t.id);
  await t.action?.run();
}
</script>

<template>
  <div class="toast-host" role="status" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="t in toasts" :key="t.id" class="toast" :class="t.kind">
        <span class="msg">{{ t.message }}</span>
        <button v-if="t.action" class="action" @click="runAction(t)">{{ t.action.label }}</button>
        <button class="close" aria-label="关闭提示" @click="dismiss(t.id)">
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M2 2l6 6M8 2l-6 6"/>
          </svg>
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-host {
  position: fixed;
  left: 50%;
  bottom: 20px;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  z-index: 200;
  pointer-events: none;
}
.toast {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 12px;
  max-width: 520px;
  padding: 9px 10px 9px 16px;
  border-radius: var(--radius);
  background: var(--toast-bg);
  color: #fff;
  font-size: 13px;
  box-shadow: var(--shadow-lg);
}
.toast.error {
  background: var(--danger);
}
.msg {
  white-space: pre-wrap;
  line-height: 1.45;
}
.action {
  color: var(--toast-action);
  font-weight: 600;
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}
.toast.error .action {
  color: #fff;
  text-decoration: underline;
}
.action:hover {
  background: rgba(255, 255, 255, 0.12);
}
.close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-sm);
  color: inherit;
  opacity: 0.7;
}
.close:hover {
  opacity: 1;
  background: rgba(255, 255, 255, 0.12);
}
.toast-enter-active,
.toast-leave-active {
  transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease);
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
