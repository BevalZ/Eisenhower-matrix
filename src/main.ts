import { createApp } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./style.css";

// Apply the last theme before the first paint; the saved setting is re-applied after load.
try {
  const theme = localStorage.getItem("theme");
  if (theme === "light" || theme === "dark") document.documentElement.setAttribute("data-theme", theme);
} catch {
  /* storage unavailable */
}

// The ball has its own small entry: no task store, board or sync listeners in this WebView.
// Mobile always renders the main app, even if an older config gives it the ball label.
const isMobile = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
const isBall = !isMobile && getCurrentWindow().label === "floating-ball";
if (isBall) {
  for (const el of [document.documentElement, document.body, document.getElementById("app")]) {
    if (el) el.style.background = "transparent";
  }
}
void (isBall ? import("./components/FloatingBall.vue") : import("./App.vue"))
  .then(({ default: component }) => createApp(component).mount("#app"));
