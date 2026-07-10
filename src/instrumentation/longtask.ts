// Dev-only main-thread long-task observer. Logs every task > THRESHOLD_MS with
// its start/end timestamps (same clock as measure()/markTiming() -> performance.now()),
// so a slow `invoke()` await can be correlated against what actually occupied the
// event loop during that window.
//
// Read the output like this: take the `invoke()` span window (e.g. 18994 -> 22973)
// and look for longtask entries whose [start, end] fall inside it. If the window is
// densely packed with longtasks, the main thread was blocked (render/JS work). If the
// window is EMPTY of longtasks, the main thread was idle and the delay is in the
// IPC/native layer, not our JS.

const THRESHOLD_MS = 50;

let installed = false;

export function installLongTaskObserver(): void {
  if (installed) return;
  if (typeof PerformanceObserver === "undefined") return;
  installed = true;

  try {
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        if (entry.duration < THRESHOLD_MS) continue;
        const startedAtMs = entry.startTime;
        const endedAtMs = entry.startTime + entry.duration;
        // `attribution` (when present) hints at the source container/frame.
        const attribution = (entry as PerformanceEntry & {
          attribution?: Array<{ name?: string; containerType?: string; containerName?: string }>;
        }).attribution;
        console.debug("[rimedit:longtask]", {
          source: "frontend",
          durationMs: entry.duration,
          startedAtMs,
          endedAtMs,
          attribution: attribution?.map((a) => ({
            name: a.name,
            containerType: a.containerType,
            containerName: a.containerName,
          })),
        });
      }
    });
    observer.observe({ type: "longtask", buffered: true });
  } catch {
    // `longtask` may be unsupported in some webviews; fail silently.
  }
}
