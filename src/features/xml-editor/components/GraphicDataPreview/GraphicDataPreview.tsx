import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useGraphicDataPreview } from "../../hooks/useGraphicDataPreview";
import type { GraphicPreviewVariant } from "../../types/graphicPreview";
import { renderDiagnostic } from "../../../../i18n/diagnostics";
import { renderGraphicPreviewLabel } from "../../lib/graphicPreviewLabels";
import styles from "./GraphicDataPreview.module.css";

interface GraphicDataPreviewProps {
  projectId?: string;
  texPath: string;
  graphicClass: string;
  maskPath?: string;
}

function getExtension(relPath: string): string {
  const dot = relPath.lastIndexOf(".");
  return dot >= 0 ? relPath.slice(dot + 1).toLowerCase() : "";
}

function previewImageSrc(variant: GraphicPreviewVariant): string {
  if (!variant.assetToken) return variant.assetUrl;
  try {
    return convertFileSrc(variant.assetToken, "rimedit-asset");
  } catch {
    return variant.assetUrl;
  }
}

export function GraphicDataPreview({
  projectId,
  texPath,
  graphicClass,
  maskPath,
}: GraphicDataPreviewProps) {
  const { t, i18n } = useTranslation("editor");
  const preview = useGraphicDataPreview(projectId, texPath, graphicClass, maskPath);
  const [imgError, setImgError] = useState(false);

  const variantId = preview.selectedVariant?.id;
  useEffect(() => {
    setImgError(false);
  }, [variantId]);

  if (!texPath.trim() && !graphicClass.trim()) return null;

  const { status, selectedVariant, selectedIndex, result, warnings, error } = preview;
  const variants = result?.variants ?? [];
  const showCarousel = variants.length > 1;
  const showDots = showCarousel && variants.length <= 8;

  function renderContent() {
    if (status === "idle" || status === "loading") {
      return (
        <div className={styles.placeholder}>
          <span className={styles.statusText}>{status === "loading" ? t("graphicPreview.loading") : ""}</span>
        </div>
      );
    }
    if (status === "error") {
      return (
        <div className={styles.placeholder}>
          <span className={styles.errorText}>{error}</span>
        </div>
      );
    }
    if (!selectedVariant) {
      return (
        <div className={styles.placeholder}>
          <span className={styles.statusText}>{t("graphicPreview.noVariantAvailable")}</span>
        </div>
      );
    }
    if (selectedVariant.missing) {
      return (
        <div className={styles.placeholder}>
          <span className={styles.statusText}>{t("graphicPreview.textureNotFound")}</span>
          <span className={styles.pathHint}>{selectedVariant.relativeTexturePath}</span>
        </div>
      );
    }
    const ext = getExtension(selectedVariant.relativeTexturePath);
    if (ext === "dds") {
      return (
        <div className={styles.placeholder}>
          <span className={styles.statusText}>{t("graphicPreview.ddsUnsupported")}</span>
        </div>
      );
    }
    if (imgError) {
      return (
        <div className={styles.placeholder}>
          <span className={styles.statusText}>{t("graphicPreview.failedToLoad")}</span>
          <span className={styles.pathHint}>
            {t("graphicPreview.sourceLabel", {
              source:
                selectedVariant.sourceLocationName ||
                selectedVariant.sourceLocationId ||
                t("graphicPreview.unknownSource"),
            })}
          </span>
          <span className={styles.pathHint}>{selectedVariant.relativeTexturePath}</span>
        </div>
      );
    }
    const imgSrc = previewImageSrc(selectedVariant);
    return (
      <img
        className={styles.image}
        src={imgSrc}
        alt={t("graphicPreview.alt", {
          graphicClass,
          label: renderGraphicPreviewLabel(selectedVariant.label, i18n),
        })}
        draggable={false}
        onError={() => setImgError(true)}
      />
    );
  }

  return (
    <div className={styles.root}>
      <div className={styles.previewRegion}>
        {renderContent()}
        {showCarousel && (
          <div className={styles.carousel}>
            <button
              className={styles.carouselButton}
              aria-label={t("graphicPreview.previousVariant")}
              onClick={preview.goPrevious}
              disabled={!preview.canGoPrevious}
            >
              <ChevronLeft size={14} />
            </button>
            <span className={styles.carouselLabel}>
              {selectedVariant ? renderGraphicPreviewLabel(selectedVariant.label, i18n) : ""}{" "}
              {selectedIndex + 1}/{variants.length}
            </span>
            <button
              className={styles.carouselButton}
              aria-label={t("graphicPreview.nextVariant")}
              onClick={preview.goNext}
              disabled={!preview.canGoNext}
            >
              <ChevronRight size={14} />
            </button>
          </div>
        )}
      </div>

      {showDots && (
        <div className={styles.dots} role="tablist">
          {variants.map((v, i) => (
            <button
              key={v.id}
              role="tab"
              aria-selected={i === selectedIndex}
              className={`${styles.dot} ${i === selectedIndex ? styles.dotActive : ""}`}
              aria-label={t("graphicPreview.showVariant", { label: renderGraphicPreviewLabel(v.label, i18n) })}
              onClick={() => preview.selectVariant(i)}
            />
          ))}
        </div>
      )}

      {warnings.length > 0 && (
        <div className={styles.warnings}>
          {warnings.map((w, i) => (
            <p key={i} className={styles.warning}>
              {renderDiagnostic(w, i18n)}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}
