import { useRef, useEffect } from "react";
import styles from "./ResizablePaneHandle.module.css";

interface ResizablePaneHandleProps {
  width: number;
  minWidth: number;
  maxWidth: number;
  defaultWidth: number;
  onChange: (width: number) => void;
}

export function ResizablePaneHandle({
  width,
  minWidth,
  maxWidth,
  defaultWidth,
  onChange,
}: ResizablePaneHandleProps) {
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);
  const draggingRef = useRef(false);

  useEffect(() => {
    return () => {
      if (draggingRef.current) {
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      }
    };
  }, []);

  function clamp(value: number) {
    return Math.max(minWidth, Math.min(maxWidth, value));
  }

  function stopDrag() {
    draggingRef.current = false;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }

  function handlePointerDown(e: React.PointerEvent<HTMLDivElement>) {
    e.currentTarget.setPointerCapture(e.pointerId);
    startXRef.current = e.clientX;
    startWidthRef.current = width;
    draggingRef.current = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }

  function handlePointerMove(e: React.PointerEvent<HTMLDivElement>) {
    if (!draggingRef.current) return;
    const delta = e.clientX - startXRef.current;
    onChange(clamp(startWidthRef.current + delta));
  }

  function handlePointerUp(e: React.PointerEvent<HTMLDivElement>) {
    if (!draggingRef.current) return;
    e.currentTarget.releasePointerCapture(e.pointerId);
    stopDrag();
  }

  function handlePointerCancel() {
    stopDrag();
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLDivElement>) {
    const step = e.shiftKey ? 48 : 16;
    switch (e.key) {
      case "ArrowLeft":
        e.preventDefault();
        onChange(clamp(width - step));
        break;
      case "ArrowRight":
        e.preventDefault();
        onChange(clamp(width + step));
        break;
      case "Home":
        e.preventDefault();
        onChange(minWidth);
        break;
      case "End":
        e.preventDefault();
        onChange(maxWidth);
        break;
      case "Enter":
        e.preventDefault();
        onChange(defaultWidth);
        break;
    }
  }

  function handleDoubleClick() {
    onChange(defaultWidth);
  }

  return (
    <div
      className={styles.root}
      role="separator"
      aria-orientation="vertical"
      aria-valuemin={minWidth}
      aria-valuemax={maxWidth}
      aria-valuenow={width}
      tabIndex={0}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerCancel}
      onKeyDown={handleKeyDown}
      onDoubleClick={handleDoubleClick}
    >
      <div className={styles.track} />
    </div>
  );
}
