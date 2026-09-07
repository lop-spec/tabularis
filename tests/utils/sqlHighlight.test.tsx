import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ colorize: vi.fn() }));
vi.mock("@monaco-editor/react", () => ({ loader: { init: vi.fn(async () => ({ editor: { colorize: mocks.colorize } })) } }));
import { formatSqlPreview, useColorizedSql } from "../../src/utils/sqlHighlight";

describe("SQL previews", () => {
  beforeEach(() => { mocks.colorize.mockReset(); });
  it("preserves trimmed preview semantics and stops after the first excess line", () => {
    expect(formatSqlPreview("\n SELECT 1 \n\n SELECT 2 \n SELECT 3 \n SELECT 4\n" + "x\n".repeat(500_000)))
      .toBe("SELECT 1\nSELECT 2\nSELECT 3 ...");
    expect(formatSqlPreview(" \n SELECT 1\n\n")).toBe("SELECT 1");
    expect(formatSqlPreview("\n\n")).toBe("");
    expect(formatSqlPreview("one", 0)).toBe(" ...");
  });
  it("deduplicates concurrent colorizations without retaining stale theme HTML", async () => {
    let finish!: (html: string) => void;
    mocks.colorize.mockImplementation(() => new Promise<string>((resolve) => { finish = resolve; }));
    const views = Array.from({ length: 20 }, () => renderHook(() => useColorizedSql("SELECT shared")));
    await waitFor(() => expect(mocks.colorize).toHaveBeenCalledTimes(1));
    await act(async () => finish('<span class="mtk1">SELECT shared</span>'));
    for (const view of views) {
      expect(view.result.current).toContain("SELECT shared");
      view.unmount();
    }
    mocks.colorize.mockResolvedValue('<span class="mtk2">SELECT shared</span>');
    const next = renderHook(() => useColorizedSql("SELECT shared"));
    await waitFor(() => expect(next.result.current).toContain("mtk2"));
    expect(mocks.colorize).toHaveBeenCalledTimes(2);
  });
  it("removes active markup before handing HTML to the component", async () => {
    mocks.colorize.mockResolvedValue('<span class="mtk1" onclick="attack()">safe</span><img src=x onerror="attack()"><script>attack()</script><a href="javascript:attack()">link</a>');
    const view = renderHook(() => useColorizedSql("SELECT unsafe"));
    await waitFor(() => expect(view.result.current).toContain("safe"));
    expect(view.result.current).not.toMatch(/onclick|onerror|script|javascript|<img|<a\s/);
  });
  it("logs failure, falls back to text and allows a later retry", async () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.colorize.mockRejectedValueOnce(new Error("colorizer offline"));
    const first = renderHook(() => useColorizedSql("SELECT retry"));
    await waitFor(() => expect(log).toHaveBeenCalled());
    expect(first.result.current).toBeNull();
    first.unmount();
    mocks.colorize.mockResolvedValue("retried");
    const second = renderHook(() => useColorizedSql("SELECT retry"));
    await waitFor(() => expect(second.result.current).toBe("retried"));
    log.mockRestore();
  });
});
