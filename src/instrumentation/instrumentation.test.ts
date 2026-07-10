import { deriveInstrumentationConfig } from "./config";
import { measure as timerMeasure, measureAsync as timerMeasureAsync } from "./timer";
import {
  isInstrumentationEnabled,
  measure as indexMeasure,
  measureAsync as indexMeasureAsync,
} from "./index";

const LS_KEY = "rimedit.instrumentation.enabled";

beforeEach(() => {
  localStorage.clear();
});

describe("deriveInstrumentationConfig", () => {
  it("returns disabled when dev is false", () => {
    expect(
      deriveInstrumentationConfig({ dev: false, envEnabled: "1", localStorageEnabled: null }),
    ).toEqual({ enabled: false });
  });

  it("returns disabled in dev when env not set", () => {
    expect(
      deriveInstrumentationConfig({ dev: true, envEnabled: undefined, localStorageEnabled: null }),
    ).toEqual({ enabled: false });
  });

  it("returns enabled when dev is true and env is '1'", () => {
    expect(
      deriveInstrumentationConfig({ dev: true, envEnabled: "1", localStorageEnabled: null }),
    ).toEqual({ enabled: true });
  });

  it("localStorage false overrides enabled env", () => {
    expect(
      deriveInstrumentationConfig({ dev: true, envEnabled: "1", localStorageEnabled: "false" }),
    ).toEqual({ enabled: false });
  });

  it("localStorage true overrides absent env", () => {
    expect(
      deriveInstrumentationConfig({ dev: true, envEnabled: undefined, localStorageEnabled: "true" }),
    ).toEqual({ enabled: true });
  });

  it("localStorage '1' enables instrumentation", () => {
    expect(
      deriveInstrumentationConfig({ dev: true, envEnabled: undefined, localStorageEnabled: "1" }),
    ).toEqual({ enabled: true });
  });

  it("localStorage '0' disables even with env enabled", () => {
    expect(
      deriveInstrumentationConfig({ dev: true, envEnabled: "1", localStorageEnabled: "0" }),
    ).toEqual({ enabled: false });
  });
});

describe("timer.measure", () => {
  it("returns the operation result", () => {
    const result = timerMeasure("op", () => 42);
    expect(result).toBe(42);
  });

  it("rethrows the operation error", () => {
    expect(() => timerMeasure("op", () => { throw new Error("x"); })).toThrow("x");
  });

  it("emits timing event even when operation throws", () => {
    const spy = vi.spyOn(console, "debug").mockImplementation(() => {});
    try { timerMeasure("op.throw", () => { throw new Error("x"); }); } catch {}
    expect(spy).toHaveBeenCalledWith(
      "[rimedit:timing]",
      expect.objectContaining({ name: "op.throw", source: "frontend" }),
    );
    spy.mockRestore();
  });
});

describe("timer.measureAsync", () => {
  it("returns the async operation result", async () => {
    const result = await timerMeasureAsync("op", () => Promise.resolve(99));
    expect(result).toBe(99);
  });

  it("rethrows the async operation error", async () => {
    await expect(
      timerMeasureAsync("op", () => Promise.reject(new Error("y"))),
    ).rejects.toThrow("y");
  });

  it("emits timing event even when operation rejects", async () => {
    const spy = vi.spyOn(console, "debug").mockImplementation(() => {});
    try { await timerMeasureAsync("op.reject", () => Promise.reject(new Error("y"))); } catch {}
    expect(spy).toHaveBeenCalledWith(
      "[rimedit:timing]",
      expect.objectContaining({ name: "op.reject", source: "frontend" }),
    );
    spy.mockRestore();
  });
});

describe("instrumentation enabled state", () => {
  it("is disabled when localStorage is set to false", () => {
    localStorage.setItem(LS_KEY, "false");
    expect(isInstrumentationEnabled()).toBe(false);
  });

  it("is enabled when localStorage is set to true", () => {
    localStorage.setItem(LS_KEY, "true");
    expect(isInstrumentationEnabled()).toBe(true);
  });

  it("measure does not call the sink when disabled", () => {
    localStorage.setItem(LS_KEY, "false");
    const spy = vi.spyOn(console, "debug").mockImplementation(() => {});
    indexMeasure("test.disabled", () => 42);
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  it("measure still returns the result when disabled", () => {
    localStorage.setItem(LS_KEY, "false");
    expect(indexMeasure("test.disabled", () => "hello")).toBe("hello");
  });

  it("measureAsync does not call the sink when disabled", async () => {
    localStorage.setItem(LS_KEY, "false");
    const spy = vi.spyOn(console, "debug").mockImplementation(() => {});
    await indexMeasureAsync("test.disabled", () => Promise.resolve(42));
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  it("measureAsync still returns the result when disabled", async () => {
    localStorage.setItem(LS_KEY, "false");
    const result = await indexMeasureAsync("test.disabled", () => Promise.resolve("world"));
    expect(result).toBe("world");
  });
});
