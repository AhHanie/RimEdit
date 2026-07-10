export type InstrumentationTags = Record<string, string | number | boolean>;

export interface InstrumentationTimingEvent {
  source: "frontend";
  name: string;
  durationMs: number;
  startedAtMs: number;
  endedAtMs: number;
  tags?: InstrumentationTags;
}
