import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ write: vi.fn() }));
vi.mock("../../src/utils/clipboard", () => ({ copyTextToClipboard: mocks.write }));
import { useCopyFeedback } from "../../src/hooks/useCopyFeedback";

describe("copy feedback lifecycle", () => {
  beforeEach(() => { vi.useFakeTimers(); mocks.write.mockReset().mockResolvedValue(undefined); });
  afterEach(() => { vi.useRealTimers(); });
  it("keeps one timer after 100 copies and none after unmount", async () => {
    const view = renderHook(() => useCopyFeedback(1500));
    for (let i = 0; i < 100; i++) await act(async () => { await view.result.current.copy(String(i)); });
    expect(view.result.current.copied).toBe(true);
    expect(vi.getTimerCount()).toBe(1);
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
  it("does not create a timer when a pending copy completes after unmount", async () => {
    let finish!: () => void;
    mocks.write.mockReturnValue(new Promise<void>((resolve) => { finish = resolve; }));
    const view = renderHook(() => useCopyFeedback());
    const pending = view.result.current.copy("pending");
    view.unmount();
    await act(async () => { finish(); await pending; });
    expect(vi.getTimerCount()).toBe(0);
  });
  it("logs errors without reporting success", async () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.write.mockRejectedValue(new Error("denied"));
    const view = renderHook(() => useCopyFeedback());
    await act(async () => { await view.result.current.copy("x"); });
    expect(view.result.current.copied).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
    expect(log).toHaveBeenCalledOnce();
    log.mockRestore();
  });
  it("reset invalidates pending copies", async () => {
    let finish!: () => void;
    mocks.write.mockReturnValue(new Promise<void>((resolve) => { finish = resolve; }));
    const view = renderHook(() => useCopyFeedback());
    const pending = view.result.current.copy("pending");
    act(() => view.result.current.reset());
    await act(async () => { finish(); await pending; });
    expect(view.result.current.copied).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
  });
});
