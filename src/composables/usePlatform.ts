import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const platform = ref("desktop");
let requested = false;

/**
 * Which platform the UI runs on, asked of the backend once per webview.
 * Desktop-only affordances (the floating ball, Tailscale peer sync) are hidden on Android
 * and iOS. Until the answer arrives the value stays "desktop", so a failed call never
 * removes a feature the user actually has.
 */
export function usePlatform() {
  if (!requested) {
    requested = true;
    invoke<string>("platform")
      .then((value) => (platform.value = value))
      .catch(() => {
        /* keep the desktop default */
      });
  }
  return {
    platform,
    isMobile: computed(() => platform.value === "android" || platform.value === "ios"),
  };
}
