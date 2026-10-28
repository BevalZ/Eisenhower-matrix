/** One read at a time; invalidations during a read require just one fresh read. */
export function createRefreshQueue<T>(read: () => Promise<T>, apply: (value: T) => void) {
  let pending: Promise<void> | undefined;
  let dirty = false;
  let holds = 0;
  let resume: (() => void) | undefined;

  function request(): Promise<void> {
    dirty = true;
    if (!pending) {
      // Coalesce synchronous bursts before starting IPC.
      pending = Promise.resolve().then(async () => {
        while (dirty) {
          if (holds) await new Promise<void>((resolve) => { resume = resolve; });
          // A new hold can be acquired before the resumed continuation runs.
          if (holds) continue;
          dirty = false;
          const value = await read();
          if (holds || dirty) {
            dirty = true;
            continue;
          }
          apply(value);
        }
      }).catch((error) => {
        dirty = false;
        throw error;
      }).finally(() => {
        pending = undefined;
        // A reactive subscriber can invalidate between apply() and this cleanup.
        if (dirty) return request();
      });
    }
    return pending;
  }

  function pause(): () => void {
    holds++;
    if (pending) dirty = true;
    let released = false;
    return () => {
      if (released) return;
      released = true;
      if (--holds === 0) {
        resume?.();
        resume = undefined;
      }
    };
  }

  return { request, pause };
}
