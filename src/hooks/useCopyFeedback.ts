import { useCallback, useEffect, useRef, useState } from "react";
import { copyTextToClipboard } from "../utils/clipboard";

// Adapted from upstream 730213a1/3ffc973f; preserve the native clipboard fallback
// and ignore pending clipboard completions after reset, replacement or unmount.
export function useCopyFeedback(resetMs = 2000) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const generation = useRef(0);
  const mounted = useRef(false);
  const clearTimer = useCallback(() => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  }, []);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current += 1;
      clearTimer();
    };
  }, [clearTimer]);
  const copy = useCallback(async (text: string) => {
    const request = ++generation.current;
    clearTimer();
    try {
      await copyTextToClipboard(text);
      if (!mounted.current || generation.current !== request) return;
      setCopied(true);
      timer.current = setTimeout(() => {
        timer.current = null;
        setCopied(false);
      }, resetMs);
    } catch (error) {
      console.error("[clipboard] Copy failed; success feedback suppressed", error);
      if (mounted.current && generation.current === request) setCopied(false);
    }
  }, [clearTimer, resetMs]);
  const reset = useCallback(() => {
    generation.current += 1;
    clearTimer();
    setCopied(false);
  }, [clearTimer]);
  return { copied, copy, reset } as const;
}
