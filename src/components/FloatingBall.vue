<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const clickedOnce = ref(false);

async function onClick() {
  if (!clickedOnce.value) {
    clickedOnce.value = true;
    return;
  }
  await invoke("restore_from_ball");
}
</script>

<template>
  <div
    class="floating-ball"
    data-tauri-drag-region
    @click="onClick"
  >
    <svg width="26" height="26" viewBox="0 0 20 20" fill="none">
      <rect x="1" y="1" width="8" height="8" rx="1.5" fill="white" opacity="0.95"/>
      <rect x="11" y="1" width="8" height="8" rx="1.5" fill="white" opacity="0.75"/>
      <rect x="1" y="11" width="8" height="8" rx="1.5" fill="white" opacity="0.75"/>
      <rect x="11" y="11" width="8" height="8" rx="1.5" fill="white" opacity="0.55"/>
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
  cursor: pointer;
  box-shadow: 0 4px 20px rgba(52, 87, 213, 0.45);
  user-select: none;
  -webkit-user-select: none;
}
.floating-ball:active {
  transform: scale(0.95);
}
</style>
