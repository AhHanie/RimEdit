import { useState, type ReactNode } from "react";
import { act } from "@testing-library/react";
import type { ProjectSettingsLoadResult } from "../features/project-settings";
import { mountApp } from "./bootstrap";

const seenInitialLocales: (string | undefined)[] = [];

vi.mock("../i18n/LocaleProvider", () => ({
  // A minimal double reproducing the real `LocaleProvider`'s locale-state initialization pattern
  // (`useState(initialLocale)` -- a directly-computed, not lazy, initializer; see
  // `src/i18n/LocaleProvider.tsx`): React only consults that initializer on a fiber's first mount,
  // and ignores it on every subsequent update to that same fiber. If `mountApp` ever rendered its
  // two `LocaleProvider` passes at the same position without distinct `key`s, this double would
  // reproduce the exact bug it's guarding against: the app-phase pass's `initialLocale` prop would
  // be silently ignored because React would treat it as an update to the startup-phase fiber
  // instead of a fresh mount. This makes the regression observable without depending on the app
  // having more than one supported locale (today only `"en"` is registered, so the real
  // component's resolved `locale` state is unobservably identical either way).
  LocaleProvider: ({
    initialLocale,
    children,
  }: {
    initialLocale?: string;
    children: ReactNode;
  }) => {
    const [seenInitialLocale] = useState(initialLocale);
    seenInitialLocales.push(seenInitialLocale);
    return <>{children}</>;
  },
}));

vi.mock("./shell/AppShell/AppShell", () => ({
  AppShell: () => <div data-testid="app-shell" />,
}));

vi.mock("./commands/windowCommands", () => ({
  signalStartupReady: () => Promise.resolve(),
}));

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

// `mountApp` also resolves the initial theme (via `window.matchMedia`) before its first render --
// jsdom doesn't implement it, so stub it the same way `AppShell.test.tsx` does for `useTheme`.
beforeEach(() => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
});

test("remounts LocaleProvider for the app render so a persisted locale from settings isn't dropped", async () => {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const deferred = createDeferred<ProjectSettingsLoadResult>();

  let mountPromise: Promise<void> | undefined;
  act(() => {
    mountPromise = mountApp({ container, projectSettingsPromise: deferred.promise });
  });

  await act(async () => {
    deferred.resolve({
      settings: {
        schemaVersion: 3,
        gameVersion: "1.6",
        locale: "en",
        locations: [],
        activeProjectId: undefined,
      },
    });
    await mountPromise;
  });

  // `React.StrictMode` double-invokes each render-phase call for impure-render detection, so every
  // actual commit shows up as a consecutive, identical pair here -- collapse those before asserting
  // on the sequence of *commits*, not raw render-body calls.
  const distinctCommits: (string | undefined)[] = [];
  for (const locale of seenInitialLocales) {
    if (distinctCommits.length === 0 || distinctCommits[distinctCommits.length - 1] !== locale) {
      distinctCommits.push(locale);
    }
  }

  // The startup-phase provider mounts first with no `initialLocale` (`undefined`). The app-phase
  // provider must mount as a genuinely fresh fiber carrying the resolved locale -- if it instead
  // reused the startup fiber (the bug), its `useState` initializer would never re-run and this
  // would read back `[undefined]`.
  expect(distinctCommits).toEqual([undefined, "en"]);
});
