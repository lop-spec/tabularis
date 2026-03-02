import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useKeybindings } from "./useKeybindings";
import { useConnectionManager } from "./useConnectionManager";

/**
 * Registers global keyboard shortcuts for navigation.
 * Must be called inside a component that is a child of KeybindingsProvider and BrowserRouter.
 */
export function useGlobalShortcuts() {
  const navigate = useNavigate();
  const { matchesShortcut, isMac } = useKeybindings();
  const { openConnections, handleSwitch } = useConnectionManager();

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't fire when typing in inputs / textareas / contenteditable
      const target = e.target as HTMLElement;
      if (
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable
      ) {
        return;
      }

      if (matchesShortcut(e, "toggle_sidebar")) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("tabularis:toggle-sidebar"));
        return;
      }

      if (matchesShortcut(e, "open_connections")) {
        e.preventDefault();
        navigate("/connections");
        return;
      }

      if (matchesShortcut(e, "new_connection")) {
        e.preventDefault();
        navigate("/connections", { state: { openNew: true } });
        return;
      }

      // Cmd/Ctrl+Shift+1–9: switch to Nth open connection (on Mac accept both ⌘ and Ctrl)
      const modifierHeld = isMac ? (e.metaKey || e.ctrlKey) : e.ctrlKey;
      if (modifierHeld && e.shiftKey && /^[1-9]$/.test(e.key)) {
        const idx = parseInt(e.key, 10) - 1;
        const target = openConnections[idx];
        if (target) {
          e.preventDefault();
          handleSwitch(target.id);
          navigate("/editor");
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [matchesShortcut, isMac, navigate, openConnections, handleSwitch]);
}
