import type { DiagnosticArgs } from "../../../lib/diagnostics";

export interface GraphicPreviewAssetResult {
  texPath: string;
  graphicClass: string;
  variants: GraphicPreviewVariant[];
  warnings: GraphicPreviewWarning[];
}

/** A non-fatal graphic-preview condition (missing Textures directory, unresolved texture, DDS
 * unsupported in the browser preview, etc). Mirrors `crate::services::graphic_preview::model::GraphicPreviewWarning`
 * -- rendered through `renderDiagnostic`, never displayed via `message` directly. */
export interface GraphicPreviewWarning {
  code: string;
  message: string;
  args?: DiagnosticArgs;
}

export type GraphicPreviewDirection = "north" | "east" | "south" | "west";

export type GraphicPreviewStackSlot = "single" | "partial" | "full";

/** A `GraphicPreviewVariant`'s display label, mirroring
 * `crate::services::graphic_preview::model::GraphicPreviewLabel` -- a translatable discriminant
 * plus typed args, never pre-assembled English text. Render via
 * `renderGraphicPreviewLabel` (`src/features/xml-editor/lib/graphicPreviewLabels.ts`), never by
 * displaying a field of this object directly. `appearanceNamed`'s `suffix` is the one exception:
 * literal text derived from an actual texture file name on disk, interpolated verbatim rather than
 * translated (see that module's docs). */
export type GraphicPreviewLabel =
  | { kind: "single" }
  | { kind: "direction"; direction: GraphicPreviewDirection }
  | { kind: "variant"; index: number; direction?: GraphicPreviewDirection }
  | { kind: "stack"; stack: GraphicPreviewStackSlot; direction?: GraphicPreviewDirection }
  | { kind: "appearance"; index: number }
  | { kind: "appearanceNamed"; suffix: string };

export interface GraphicPreviewVariant {
  id: string;
  label: GraphicPreviewLabel;
  role: string;
  sourceLocationId: string;
  sourceLocationName: string;
  relativeTexturePath: string;
  assetUrl: string;
  assetToken?: string;
  missing?: boolean;
}
