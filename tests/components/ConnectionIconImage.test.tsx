import { StrictMode } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (path: string) => `tauri://${path}` }));
vi.mock("@tauri-apps/api/path", () => ({ appDataDir: async () => "/app", join: async (...parts: string[]) => parts.join("/") }));
import { ConnectionIconImage } from "../../src/components/ConnectionIconImage";

describe("connection icon lifecycle", () => {
  it("falls back on image errors even after StrictMode effect replay", async () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    const { container, rerender } = render(<StrictMode><ConnectionIconImage path="a.png" size={16} fallback={<span>fallback</span>} /></StrictMode>);
    await waitFor(() => expect(container.querySelector("img")).not.toBeNull());
    fireEvent.error(container.querySelector("img")!);
    expect(screen.getByText("fallback")).toBeInTheDocument();
    expect(log).toHaveBeenCalledOnce();
    rerender(<StrictMode><ConnectionIconImage path="b.png" size={16} fallback={<span>fallback</span>} /></StrictMode>);
    await waitFor(() => expect(container.querySelector("img")?.src).toContain("b.png"));
    log.mockRestore();
  });
});
