// i18next initialization. English resources are bundled and loaded synchronously
// (`initImmediate: false`) so translated text is available on first render.

import i18next, { type i18n as I18nInstance } from "i18next";
import { initReactI18next } from "react-i18next";
import { defaultNamespace, enResources, namespaces } from "./generated/translation-keys";
import { FALLBACK_LOCALE } from "./locale";

export { defaultNamespace, enResources, namespaces };

/** Creates a standalone, fully initialized i18next instance. Used by the app-wide
 * singleton (`initI18n`) and by test utilities that need an isolated instance. */
export function createI18nInstance(): I18nInstance {
  const instance = i18next.createInstance();
  // `.init()` completes synchronously here: all resources are supplied upfront
  // (no backend/loader plugin), so `instance.t(...)` is usable immediately after
  // this call returns without awaiting the returned promise.
  void instance.use(initReactI18next).init({
    lng: FALLBACK_LOCALE,
    fallbackLng: FALLBACK_LOCALE,
    supportedLngs: [FALLBACK_LOCALE],
    defaultNS: defaultNamespace,
    ns: [...namespaces],
    resources: {
      [FALLBACK_LOCALE]: enResources,
    },
    interpolation: {
      escapeValue: false,
    },
    returnEmptyString: false,
  });
  return instance;
}

let sharedInstance: I18nInstance | null = null;

/** Returns the app-wide i18next singleton, creating it on first call. */
export function initI18n(): I18nInstance {
  if (!sharedInstance) {
    sharedInstance = createI18nInstance();
  }
  return sharedInstance;
}
