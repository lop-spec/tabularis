import { useState, useRef, useEffect } from "react";

const MIN_WIDTH = 150;
const MAX_WIDTH = 600;
const DEFAULT_WIDTH = 256;
const STORAGE_KEY = "tabularis_sidebar_width";

export const useSidebarResize = () => {
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved ? parseInt(saved, 10) : DEFAULT_WIDTH;
  });
  const isDragging = useRef(false);

  const startResize = (e: React.MouseEvent) => {
    e.preventDefault();
    isDragging.current = true;
    document.body.style.cursor = "col-resize";

    const handleResize = (e: MouseEvent) => {
      if (!isDragging.current) return;
      const newWidth = e.clientX - 64; // Subtract primary sidebar width (w-16 = 64px)
      if (newWidth > MIN_WIDTH && newWidth < MAX_WIDTH) {
        setSidebarWidth(newWidth);
      }
    };

    const stopResize = () => {
      isDragging.current = false;
      document.body.style.cursor = "default";
      document.removeEventListener("mousemove", handleResize);
      document.removeEventListener("mouseup", stopResize);
      // localStorage is handled by useEffect
    };

    document.addEventListener("mousemove", handleResize);
    document.addEventListener("mouseup", stopResize);
  };
  
  // Persist width on change as well to be safe
  useEffect(() => {
      localStorage.setItem(STORAGE_KEY, sidebarWidth.toString());
  }, [sidebarWidth]);

  return { sidebarWidth, startResize };
};
