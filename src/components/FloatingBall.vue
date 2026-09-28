<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Moving farther than this (CSS px) with the button held turns a press into a drag. */
const DRAG_THRESHOLD = 4;

const pressed = ref(false);
let start: { x: number; y: number } | null = null;

// A native drag region starts the OS move loop on mousedown, and on Windows that loop
// swallows the mouseup, so a plain click could never restore the window. Start the
// drag only once the pointer moves; a release without moving is a click.
function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return;
  start = { x: e.clientX, y: e.clientY };
  pressed.value = true;
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

function onMouseMove(e: MouseEvent) {
  if (!start) return;
  if (!(e.buttons & 1)) {
    endPress(); // released somewhere we didn't hear about
    return;
  }
  if (Math.hypot(e.clientX - start.x, e.clientY - start.y) < DRAG_THRESHOLD) return;
  endPress();
  getCurrentWindow().startDragging();
}

function onMouseUp(e: MouseEvent) {
  const click = start !== null && e.button === 0;
  endPress();
  if (click) restore();
}

function endPress() {
  start = null;
  pressed.value = false;
  window.removeEventListener("mousemove", onMouseMove);
  window.removeEventListener("mouseup", onMouseUp);
}

function restore() {
  invoke("restore_from_ball");
}

// Enter / Space; mouse clicks (detail > 0) are handled on mouseup.
function onClick(e: MouseEvent) {
  if (e.detail === 0) restore();
}
</script>

<template>
  <div class="ball-window" @contextmenu.prevent>
    <button
      type="button"
      class="floating-ball"
      :class="{ pressed }"
      title="单击打开主窗口，按住可拖动"
      aria-label="打开主窗口"
      @mousedown="onMouseDown"
      @click="onClick"
    >
      <svg width="26" height="26" viewBox="0 0 20 20" fill="none" aria-hidden="true">
        <rect x="1" y="1" width="8" height="8" rx="1.5" fill="white" opacity="0.95"/>
        <rect x="11" y="1" width="8" height="8" rx="1.5" fill="white" opacity="0.75"/>
        <rect x="1" y="11" width="8" height="8" rx="1.5" fill="white" opacity="0.75"/>
        <rect x="11" y="11" width="8" height="8" rx="1.5" fill="white" opacity="0.55"/>
      </svg>
    </button>
  </div>
</template>

<style scoped>
.ball-window {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
/* 56px ball in the 64px window: the shadow has to fit in the 4px gap or it gets cut off */
.floating-ball {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: linear-gradient(135deg, #3457d5, #6c5ce7);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  user-select: none;
  -webkit-user-select: none;
}
/* The OS move loop can leave :active stuck after a drag; press feedback uses .pressed */
.floating-ball:active {
  transform: none;
}
.floating-ball.pressed {
  transform: scale(0.95);
}
</style>
