export interface GraphicPreviewAssetResult {
  texPath: string;
  graphicClass: string;
  variants: GraphicPreviewVariant[];
  warnings: string[];
}

export interface GraphicPreviewVariant {
  id: string;
  label: string;
  role: string;
  sourceLocationId: string;
  sourceLocationName: string;
  relativeTexturePath: string;
  assetUrl: string;
  assetToken?: string;
  missing?: boolean;
}
