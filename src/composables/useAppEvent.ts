import { onScopeDispose } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Own asynchronous Tauri listeners even if the component closes before listen resolves. */
export function useAppEvent(event: string, callback: () => void) {
  let disposed = false;
  let unlisten: UnlistenFn | undefined;
  void listen(event, () => { if (!disposed) callback(); }).then((stop) => {
    if (disposed) stop();
    else unlisten = stop;
  }).catch((error) => {
    if (!disposed) console.error(`Unable to listen for ${event}`, error);
  });
  onScopeDispose(() => {
    disposed = true;
    unlisten?.();
  });
}
