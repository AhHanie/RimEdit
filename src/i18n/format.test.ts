import { describe, it, expect } from "vitest";
import {
  formatDate,
  formatDateTime,
  formatFileSize,
  formatNumber,
  formatPercent,
  formatRelativeTime,
  formatTime,
} from "./format";

describe("formatNumber", () => {
  it("formats using the English default locale", () => {
    expect(formatNumber(1234.5)).toBe("1,234.5");
  });

  it("accepts Intl.NumberFormatOptions", () => {
    expect(formatNumber(1234.5, { maximumFractionDigits: 0 })).toBe("1,235");
  });

  it("accepts an explicit locale", () => {
    expect(formatNumber(1234.5, { locale: "en" })).toBe("1,234.5");
  });
});

describe("formatPercent", () => {
  it("formats a fraction as a percentage", () => {
    expect(formatPercent(0.5)).toBe("50%");
  });
});

describe("formatDate", () => {
  it("formats a date deterministically for English", () => {
    const date = new Date(Date.UTC(2026, 0, 15));
    expect(formatDate(date, { timeZone: "UTC" })).toBe("Jan 15, 2026");
  });
});

describe("formatTime", () => {
  it("formats a time", () => {
    const date = new Date(Date.UTC(2026, 0, 15, 13, 30));
    expect(formatTime(date, { timeZone: "UTC" })).toBe("1:30 PM");
  });
});

describe("formatDateTime", () => {
  it("formats date and time together", () => {
    const date = new Date(Date.UTC(2026, 0, 15, 13, 30));
    expect(formatDateTime(date, { timeZone: "UTC" })).toBe("Jan 15, 2026, 1:30 PM");
  });
});

describe("formatRelativeTime", () => {
  it("formats a future relative day", () => {
    expect(formatRelativeTime(1, "day")).toBe("tomorrow");
  });

  it("formats a past relative day", () => {
    expect(formatRelativeTime(-1, "day")).toBe("yesterday");
  });
});

describe("formatFileSize", () => {
  it("formats sub-kilobyte sizes as bytes", () => {
    expect(formatFileSize(512)).toBe("512 byte");
  });

  it("formats kilobyte-scale sizes", () => {
    expect(formatFileSize(1536)).toBe("1.5 kB");
  });

  it("formats megabyte-scale sizes", () => {
    expect(formatFileSize(5 * 1024 * 1024)).toBe("5 MB");
  });

  it("formats gigabyte-scale sizes", () => {
    expect(formatFileSize(2 * 1024 * 1024 * 1024)).toBe("2 GB");
  });
});
