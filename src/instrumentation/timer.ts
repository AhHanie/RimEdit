import type { InstrumentationTags, InstrumentationTimingEvent } from "./types";

function emitEvent(event: InstrumentationTimingEvent): void {
  try {
    console.debug("[rimedit:timing]", event);
  } catch {
    // swallow logging failures
  }
}

export function measure<T>(name: string, operation: () => T, tags?: InstrumentationTags): T {
  const startedAtMs = performance.now();
  try {
    return operation();
  } finally {
    const endedAtMs = performance.now();
    emitEvent({
      source: "frontend",
      name,
      durationMs: endedAtMs - startedAtMs,
      startedAtMs,
      endedAtMs,
      tags,
    });
  }
}

export async function measureAsync<T>(
  name: string,
  operation: () => Promise<T>,
  tags?: InstrumentationTags,
): Promise<T> {
  const startedAtMs = performance.now();
  try {
    return await operation();
  } finally {
    const endedAtMs = performance.now();
    emitEvent({
      source: "frontend",
      name,
      durationMs: endedAtMs - startedAtMs,
      startedAtMs,
      endedAtMs,
      tags,
    });
  }
}

export function markTiming(name: string, durationMs: number, tags?: InstrumentationTags): void {
  const endedAtMs = performance.now();
  emitEvent({
    source: "frontend",
    name,
    durationMs,
    startedAtMs: endedAtMs - durationMs,
    endedAtMs,
    tags,
  });
}
