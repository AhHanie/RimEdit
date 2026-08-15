import { getProjectSettings } from "./features/project-settings/api/projectSettings";
import { mountApp } from "./app/bootstrap";

vi.mock("./features/project-settings/api/projectSettings", () => ({
  // Never resolves -- this test only asserts the call itself and what gets forwarded to
  // `mountApp`, not startup's async settling behavior (covered by `src/app/bootstrap.test.tsx`).
  getProjectSettings: vi.fn(() => new Promise(() => {})),
  updateAppLocale: vi.fn(),
}));

vi.mock("./app/bootstrap", () => ({
  mountApp: vi.fn(() => Promise.resolve()),
}));

test("calls getProjectSettings exactly once and forwards the same promise to mountApp", async () => {
  document.body.innerHTML = '<div id="root"></div>';

  await import("./main");

  expect(getProjectSettings).toHaveBeenCalledTimes(1);
  expect(mountApp).toHaveBeenCalledTimes(1);

  const options = vi.mocked(mountApp).mock.calls[0][0];
  expect(options.container).toBe(document.getElementById("root"));
  expect(options.projectSettingsPromise).toBe(vi.mocked(getProjectSettings).mock.results[0]!.value);
  expect(typeof options.persistLocale).toBe("function");
  expect(typeof options.startedAtMs).toBe("number");
});
