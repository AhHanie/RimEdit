// Translates a `GraphicPreviewVariant.label` (see `types/graphicPreview.ts`'s
// `GraphicPreviewLabel`) into display text. The backend only ever ships a translatable
// discriminant + typed args -- never assembled English -- so every kind here is resolved through
// `i18n.t(...)`, mirroring how `src/i18n/diagnostics.ts`'s `renderDiagnosticSeverity` translates a
// closed backend-sourced string instead of displaying it directly.
//
// Takes the raw `i18n` instance (as `useTranslation()` returns alongside `t`), not a
// namespace-bound `TFunction` -- this module's keys cross namespaces via an explicit `editor:`
// prefix and are resolved dynamically (not one of the generated literal-key overloads the typed
// `TFunction` from a namespace-scoped `useTranslation("editor")` call statically accepts), the same
// "dynamic key" overload `src/i18n/diagnostics.ts`'s `translate()` helper documents and uses.

import type { i18n as I18nInstance } from "i18next";
import type { GraphicPreviewDirection, GraphicPreviewLabel } from "../types/graphicPreview";

function translate(i18n: I18nInstance, key: string, defaultValue: string, options?: Record<string, unknown>): string {
  return String(i18n.t(key, defaultValue, options));
}

function directionLabel(i18n: I18nInstance, direction: GraphicPreviewDirection): string {
  const fallback: Record<GraphicPreviewDirection, string> = {
    north: "North",
    east: "East",
    south: "South",
    west: "West",
  };
  return translate(i18n, `editor:graphicPreview.label.direction.${direction}`, fallback[direction]);
}

/** Renders a `GraphicPreviewLabel` to translated display text. `appearanceNamed`'s `suffix` is
 * literal text derived from an actual texture file name on disk (e.g. `"Damaged"` from
 * `Blocks_Damaged.png`) -- not a fixed UI vocabulary entry, so it is interpolated verbatim rather
 * than looked up in the translation catalog, the same way a diagnostic's literal `args` (a field
 * name, a def name) are interpolated rather than translated. */
export function renderGraphicPreviewLabel(label: GraphicPreviewLabel, i18n: I18nInstance): string {
  switch (label.kind) {
    case "single":
      return translate(i18n, "editor:graphicPreview.label.single", "Single");
    case "direction":
      return directionLabel(i18n, label.direction);
    case "variant":
      return label.direction
        ? translate(i18n, "editor:graphicPreview.label.variantDirectional", "Variant {{index}} {{direction}}", {
            index: label.index,
            direction: directionLabel(i18n, label.direction),
          })
        : translate(i18n, "editor:graphicPreview.label.variant", "Variant {{index}}", { index: label.index });
    case "stack": {
      const stackFallback: Record<string, string> = {
        single: "Stack 1",
        partial: "Stack partial",
        full: "Stack full",
      };
      const stackWord = translate(
        i18n,
        `editor:graphicPreview.label.stack.${label.stack}`,
        stackFallback[label.stack],
      );
      return label.direction
        ? translate(i18n, "editor:graphicPreview.label.stackDirectional", "{{stack}} {{direction}}", {
            stack: stackWord,
            direction: directionLabel(i18n, label.direction),
          })
        : stackWord;
    }
    case "appearance":
      return translate(i18n, "editor:graphicPreview.label.appearance", "Appearance {{index}}", {
        index: label.index,
      });
    case "appearanceNamed":
      return label.suffix;
  }
}
