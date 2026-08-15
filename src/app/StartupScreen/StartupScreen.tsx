import { useTranslation } from "react-i18next";
import styles from "./StartupScreen.module.css";

/**
 * Immediate React-rendered replacement for `index.html`'s inline startup fallback. Mounted before
 * `getProjectSettings()` resolves, under the default (English) `LocaleProvider`, so it must not
 * depend on the persisted locale, project
 * scanning, the schema catalog, or any other Tauri command -- its only job is to be visible
 * immediately and stay cheap enough that it can't itself reintroduce the startup pause.
 */
export function StartupScreen() {
  const { t } = useTranslation("common");

  return (
    <div className={styles.root}>
      <div className={styles.panel}>
        <span className={styles.title}>{t("app.name")}</span>
        <div className={styles.spinner} aria-hidden="true" />
        <p className={styles.message} role="status" aria-live="polite">
          {t("startup.loading")}
        </p>
      </div>
    </div>
  );
}
