<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const ball = ref<HTMLElement>();
const dragging = ref(false);
const hidden = ref(false);
let startX = 0, startY = 0, startWinX = 0, startWinY = 0;

async function onPointerDown(e: PointerEvent) {
  dragging.value = true;
  hidden.value = false;
  startX = e.clientX;
  startY = e.clientY;
  const win = getCurrentWindow();
  const pos = await win.outerPosition();
  startWinX = pos.x;
  startWinY = pos.y;
  ball.value?.setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  if (!dragging.value) return;
  const dx = e.clientX - startX;
  const dy = e.clientY - startY;
  getCurrentWindow().setPosition({ x: startWinX + dx, y: startWinY + dy });
}

async function onPointerUp(e: PointerEvent) {
  if (!dragging.value) return;
  dragging.value = false;
  ball.value?.releasePointerCapture(e.pointerId);

  // Snap to nearest edge
  const win = getCurrentWindow();
  const pos = await win.outerPosition();
  const size = await win.outerSize();
  const monitor = await win.currentMonitor();
  const m = monitor!;
  const winX = pos.x - m.position.x;
  const winY = pos.y - m.position.y;
  const midX = winX + size.width / 2;
  const screenW = m.size.width;

  if (midX < screenW / 2) {
    // Snap left
    win.setPosition({ x: m.position.x + 8, y: pos.y });
    setTimeout(() => { hidden.value = true; }, 300);
  } else {
    // Snap right
    win.setPosition({ x: m.position.x + m.size.width - size.width - 8, y: pos.y });
    setTimeout(() => { hidden.value = true; }, 300);
  }
}

async function onClick() {
  if (dragging.value) return;
  if (hidden.value) {
    // First click: unhide
    hidden.value = false;
    return;
  }
  await invoke("restore_from_ball");
}

onMounted(() => {
  // Unhide when mouse approaches (for edge-hidden ball)
  getCurrentWindow().setIgnoreCursorEvents(true);
});
</script>

<template>
  <div
    ref="ball"
    class="floating-ball"
    :class="{ dragging, hidden }"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @click="onClick"
  >
    <svg width="28" height="28" viewBox="0 0 20 20" fill="none">
      <rect x="1" y="1" width="8" height="8" rx="1.5" fill="white" opacity="0.95"/>
      <rect x="11" y="1" width="8" height="8" rx="1.5" fill="white" opacity="0.7"/>
      <rect x="1" y="11" width="8" height="8" rx="1.5" fill="white" opacity="0.7"/>
      <rect x="11" y="11" width="8" height="8" rx="1.5" fill="white" opacity="0.5"/>
    </svg>
  </div>
</template>

<style scoped>
.floating-ball {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: linear-gradient(135deg, #3457d5, #6c5ce7);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: grab;
  box-shadow: 0 4px 20px rgba(52, 87, 213, 0.4);
  transition: transform 0.25s ease, opacity 0.25s ease, box-shadow 0.25s ease;
  user-select: none;
}
.floating-ball:hover {
  transform: scale(1.08);
  box-shadow: 0 6px 28px rgba(52, 87, 213, 0.55);
}
.floating-ball.dragging {
  cursor: grabbing;
  transform: scale(1.12);
}
.floating-ball.hidden {
  opacity: 0.35;
  transform: scale(0.85);
}
</style>
