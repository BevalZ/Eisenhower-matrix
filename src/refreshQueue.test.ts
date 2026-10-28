import { describe, expect, it, vi } from "vitest";
import { createRefreshQueue } from "./refreshQueue";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe("refresh queue", () => {
  it("coalesces a synchronous event burst into one read", async () => {
    const read = vi.fn(async () => 1);
    const apply = vi.fn();
    const queue = createRefreshQueue(read, apply);
    await Promise.all(Array.from({ length: 100 }, () => queue.request()));
    expect(read).toHaveBeenCalledTimes(1);
    expect(apply).toHaveBeenCalledExactlyOnceWith(1);
  });

  it("discards an invalidated in-flight result and runs only one trailing read", async () => {
    const first = deferred<number>();
    const read = vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(2);
    const apply = vi.fn();
    const queue = createRefreshQueue(read, apply);
    const request = queue.request();
    await Promise.resolve();
    const requests = Array.from({ length: 100 }, () => queue.request());
    first.resolve(1);
    await Promise.all([request, ...requests]);
    expect(read).toHaveBeenCalledTimes(2);
    expect(apply).toHaveBeenCalledExactlyOnceWith(2);
  });

  it("waits for every drag/write hold and releases idempotently", async () => {
    const read = vi.fn(async () => 1);
    const queue = createRefreshQueue(read, vi.fn());
    const releaseDrag = queue.pause();
    const releaseWrite = queue.pause();
    const request = queue.request();
    await Promise.resolve();
    releaseDrag();
    releaseDrag();
    await Promise.resolve();
    expect(read).not.toHaveBeenCalled();
    releaseWrite();
    await request;
    expect(read).toHaveBeenCalledTimes(1);
  });

  it("does not apply a pre-write snapshot even if the write finishes before the read", async () => {
    const first = deferred<number>();
    const read = vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(2);
    const apply = vi.fn();
    const queue = createRefreshQueue(read, apply);
    const request = queue.request();
    await Promise.resolve();
    const release = queue.pause();
    release();
    first.resolve(1);
    await request;
    expect(apply).toHaveBeenCalledExactlyOnceWith(2);
  });

  it("retries after a failed read without leaving the queue stuck", async () => {
    const read = vi.fn().mockRejectedValueOnce(new Error("offline")).mockResolvedValue(2);
    const apply = vi.fn();
    const queue = createRefreshQueue(read, apply);
    await expect(queue.request()).rejects.toThrow("offline");
    await queue.request();
    expect(apply).toHaveBeenCalledExactlyOnceWith(2);
  });

  it("retains invalidations queued by a reactive subscriber after apply", async () => {
    let value = 0;
    const read = vi.fn(async () => ++value);
    const queue = createRefreshQueue(read, () => {
      if (value === 1) queueMicrotask(() => { void queue.request(); });
    });
    await queue.request();
    expect(read).toHaveBeenCalledTimes(2);
  });
});
