import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

// Apply the last theme before the first paint; the saved setting is re-applied after load.
try {
  const theme = localStorage.getItem("theme");
  if (theme === "light" || theme === "dark") document.documentElement.setAttribute("data-theme", theme);
} catch {
  /* storage unavailable */
}

createApp(App).mount("#app");
