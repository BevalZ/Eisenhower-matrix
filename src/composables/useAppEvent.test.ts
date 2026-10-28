import { afterEach, describe, expect, it, vi } from "vitest";
import { effectScope } from "vue";
import { listen } from "@tauri-apps/api/event";
import { useAppEvent } from "./useAppEvent";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
afterEach(() => vi.resetAllMocks());

describe("Tauri listener ownership", () => {
  it("unsubscribes when its scope stops", async () => {
    const stop = vi.fn();
    vi.mocked(listen).mockResolvedValue(stop);
    const scope = effectScope();
    scope.run(() => useAppEvent("tasks-changed", vi.fn()));
    await Promise.resolve();
    scope.stop();
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it("cleans up a late registration and ignores callbacks after disposal", async () => {
    let finish!: (stop: () => void) => void;
    const stop = vi.fn();
    const callback = vi.fn();
    vi.mocked(listen).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const scope = effectScope();
    scope.run(() => useAppEvent("tasks-changed", callback));
    scope.stop();
    const handler = vi.mocked(listen).mock.calls[0][1];
    handler({ event: "tasks-changed", id: 1, payload: null });
    finish(stop);
    await Promise.resolve();
    expect(stop).toHaveBeenCalledTimes(1);
    expect(callback).not.toHaveBeenCalled();
  });

  it("handles rejected registrations without an unhandled promise", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    vi.mocked(listen).mockRejectedValue(new Error("unavailable"));
    const scope = effectScope();
    scope.run(() => useAppEvent("tasks-changed", vi.fn()));
    await Promise.resolve();
    await Promise.resolve();
    expect(error).toHaveBeenCalledTimes(1);
    scope.stop();
    error.mockRestore();
  });
});
