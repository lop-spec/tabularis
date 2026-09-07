import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ navigate: vi.fn(), matches: vi.fn(() => true), switchConnection: vi.fn() }));
vi.mock("react-router-dom", () => ({ useNavigate: () => mocks.navigate }));
vi.mock("../../src/hooks/useConnectionManager", () => ({ useConnectionManager: () => ({ openConnections: [], handleSwitch: mocks.switchConnection }) }));
vi.mock("../../src/hooks/useKeybindings", () => ({ useKeybindings: () => ({ matchesShortcut: mocks.matches, isMac: false }) }));
import { useGlobalShortcuts } from "../../src/hooks/useGlobalShortcuts";
import { isTextCompositionKeyEvent } from "../../src/utils/keyboardEvents";

describe("IME shortcut protection", () => {
  it.each([
    { key: "a", isComposing: true }, { key: "Dead" }, { key: "Process" },
    { key: "Unidentified" }, { key: "a", keyCode: 229 },
  ])("leaves composition event %j untouched", (init) => {
    mocks.matches.mockClear();
    renderHook(() => useGlobalShortcuts());
    const event = new KeyboardEvent("keydown", { ...init, ctrlKey: true, cancelable: true });
    expect(isTextCompositionKeyEvent(event)).toBe(true);
    window.dispatchEvent(event);
    expect(mocks.matches).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });
  it("still handles ordinary shortcuts", () => {
    mocks.matches.mockClear();
    renderHook(() => useGlobalShortcuts());
    const event = new KeyboardEvent("keydown", { key: "b", ctrlKey: true, cancelable: true });
    document.body.dispatchEvent(event);
    window.dispatchEvent(event);
    expect(isTextCompositionKeyEvent(event)).toBe(false);
    expect(mocks.matches).toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });
});
