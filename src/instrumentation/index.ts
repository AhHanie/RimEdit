import { deriveInstrumentationConfig } from "./config";
import {
  measure as _measure,
  measureAsync as _measureAsync,
  markTiming as _markTiming,
} from "./timer";
import type { InstrumentationTags } from "./types";

export type { InstrumentationTags };
export type { InstrumentationTimingEvent } from "./types";
export { installLongTaskObserver } from "./longtask";

const LOCAL_STORAGE_KEY = "rimedit.instrumentation.enabled";

let traceCounter = 0;

export function generateTraceId(): string {
  return `preview-${Date.now().toString(36)}-${++traceCounter}`;
}

function readLocalStorageOverride(): string | null {
  try {
    return localStorage.getItem(LOCAL_STORAGE_KEY);
  } catch {
    return null;
  }
}

export function isInstrumentationEnabled(): boolean {
  return deriveInstrumentationConfig({
    dev: import.meta.env.DEV,
    envEnabled: import.meta.env.VITE_RIMEDIT_INSTRUMENTATION,
    localStorageEnabled: readLocalStorageOverride(),
  }).enabled;
}

export function setInstrumentationEnabled(enabled: boolean): void {
  if (!import.meta.env.DEV) return;
  try {
    localStorage.setItem(LOCAL_STORAGE_KEY, String(enabled));
  } catch {
    // swallow
  }
}

export function measure<T>(name: string, operation: () => T, tags?: InstrumentationTags): T {
  if (!isInstrumentationEnabled()) return operation();
  return _measure(name, operation, tags);
}

export async function measureAsync<T>(
  name: string,
  operation: () => Promise<T>,
  tags?: InstrumentationTags,
): Promise<T> {
  if (!isInstrumentationEnabled()) return operation();
  return _measureAsync(name, operation, tags);
}

export function markTiming(name: string, durationMs: number, tags?: InstrumentationTags): void {
  if (!isInstrumentationEnabled()) return;
  _markTiming(name, durationMs, tags);
}
