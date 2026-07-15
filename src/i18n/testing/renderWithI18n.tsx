// Test utility for components/hooks that call `useTranslation` or `useLocale`.
// Renders a fresh i18next instance per call so tests never share locale state.

import { render } from "@testing-library/react";
import type { RenderOptions, RenderResult } from "@testing-library/react";
import type { ReactElement, ReactNode } from "react";
import { createI18nInstance } from "../index";
import { LocaleProvider } from "../LocaleProvider";
import type { LocaleProviderProps } from "../LocaleProvider";

export type RenderWithI18nOptions = RenderOptions & {
  providerProps?: Omit<LocaleProviderProps, "children" | "i18nInstance">;
};

export function renderWithI18n(ui: ReactElement, options: RenderWithI18nOptions = {}): RenderResult {
  const { providerProps, ...renderOptions } = options;
  const i18nInstance = createI18nInstance();

  // Uses RTL's `wrapper` option (rather than embedding `<LocaleProvider>` directly around `ui`)
  // so that the returned `rerender(...)` also re-applies the provider -- a caller that rerenders
  // with a differently-rooted element (e.g. swapping which context provider wraps it) would
  // otherwise silently remount everything below a bare `{ui}` wrap, since React reconciles by
  // element type at each position and the provider itself would vanish from the tree on rerender.
  function Wrapper({ children }: { children?: ReactNode }) {
    return (
      <LocaleProvider i18nInstance={i18nInstance} {...providerProps}>
        {children}
      </LocaleProvider>
    );
  }

  return render(ui, { wrapper: Wrapper, ...renderOptions });
}
