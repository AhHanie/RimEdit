import { act, screen, within } from "@testing-library/react";
import type { ProjectSettingsLoadResult } from "../features/project-settings";
import { mountApp } from "./bootstrap";

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

let capturedAppShellProps: {
  initialProjectSettingsPromise?: Promise<ProjectSettingsLoadResult>;
} | null = null;

vi.mock("./shell/AppShell/AppShell", () => ({
  AppShell: (props: { initialProjectSettingsPromise?: Promise<ProjectSettingsLoadResult> }) => {
    capturedAppShellProps = props;
    return <div data-testid="app-shell" />;
  },
}));

const signalStartupReadyMock = vi.fn().mockResolvedValue(undefined);

vi.mock("./commands/windowCommands", () => ({
  signalStartupReady: () => signalStartupReadyMock(),
}));

// `resolveInitialTheme` (used by `mountApp` to set `data-theme` before the first render) reads
// `window.matchMedia` -- jsdom implements neither, so stub it the same way
// `src/app/shell/AppShell/AppShell.test.tsx` does for `useTheme`.
function stubMatchMedia(matchesDark: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesDark,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

beforeEach(() => {
  capturedAppShellProps = null;
  localStorage.clear();
  stubMatchMedia(false);
  signalStartupReadyMock.mockClear();
});

afterEach(() => {
  document.body.replaceChildren();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.style.colorScheme = "";
});

const nonDefaultSettings: ProjectSettingsLoadResult = {
  settings: {
    schemaVersion: 4,
    gameVersion: "1.6",
    locale: "en",
    locations: [],
    activeProjectId: undefined,
    saveBackupsEnabled: false,
  },
};

function createContainer(): HTMLElement {
  const container = document.createElement("div");
  document.body.appendChild(container);
  return container;
}

test("shows an accessible loading status before settings resolve, with no actionable controls", () => {
  const container = createContainer();
  const deferred = createDeferred<ProjectSettingsLoadResult>();

  act(() => {
    void mountApp({ container, projectSettingsPromise: deferred.promise });
  });

  expect(within(container).getByRole("status")).not.toBeNull();
  expect(container.querySelector("button")).toBeNull();
  expect(container.querySelector("a")).toBeNull();
  expect(container.children).toHaveLength(1);
  expect(screen.queryByTestId("app-shell")).toBeNull();
});

test("reveals the (startup-hidden) native window right after the startup screen paints", () => {
  const container = createContainer();
  const deferred = createDeferred<ProjectSettingsLoadResult>();

  act(() => {
    void mountApp({ container, projectSettingsPromise: deferred.promise });
  });

  expect(signalStartupReadyMock).toHaveBeenCalledTimes(1);
});

test("does not let a signalStartupReady rejection (e.g. outside a Tauri shell) break startup", async () => {
  signalStartupReadyMock.mockRejectedValueOnce(new Error("no native window"));
  const container = createContainer();
  const deferred = createDeferred<ProjectSettingsLoadResult>();
  let mountPromise: Promise<void> | undefined;

  act(() => {
    mountPromise = mountApp({ container, projectSettingsPromise: deferred.promise });
  });

  await act(async () => {
    deferred.resolve(nonDefaultSettings);
    await mountPromise;
  });

  expect(within(container).getByTestId("app-shell")).not.toBeNull();
});

test("replaces the startup screen with the app once settings resolve, forwarding the same promise", async () => {
  const container = createContainer();
  const deferred = createDeferred<ProjectSettingsLoadResult>();
  let mountPromise: Promise<void> | undefined;

  act(() => {
    mountPromise = mountApp({ container, projectSettingsPromise: deferred.promise });
  });

  await act(async () => {
    deferred.resolve(nonDefaultSettings);
    await mountPromise;
  });

  expect(within(container).queryByRole("status")).toBeNull();
  expect(within(container).getByTestId("app-shell")).not.toBeNull();
  expect(capturedAppShellProps?.initialProjectSettingsPromise).toBe(deferred.promise);
});

test("applies the persisted/system dark theme to the document before the startup screen paints", () => {
  stubMatchMedia(true);
  const container = createContainer();
  const deferred = createDeferred<ProjectSettingsLoadResult>();

  act(() => {
    void mountApp({ container, projectSettingsPromise: deferred.promise });
  });

  expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  expect(document.documentElement.style.colorScheme).toBe("dark");
});

test("falls back to the default app render on a settings-load rejection, without an unhandled rejection", async () => {
  const container = createContainer();
  const deferred = createDeferred<ProjectSettingsLoadResult>();
  let mountPromise: Promise<void> | undefined;

  act(() => {
    mountPromise = mountApp({ container, projectSettingsPromise: deferred.promise });
  });

  await act(async () => {
    deferred.reject(new Error("settings read failed"));
    await mountPromise;
  });

  expect(within(container).queryByRole("status")).toBeNull();
  expect(within(container).getByTestId("app-shell")).not.toBeNull();
  expect(capturedAppShellProps?.initialProjectSettingsPromise).toBe(deferred.promise);
});
