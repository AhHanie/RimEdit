import { useTranslation } from "react-i18next";
import { File } from "lucide-react";
import type { OpenFileTab } from "../../types";
import styles from "./UnsupportedFilePane.module.css";

interface UnsupportedFilePaneProps {
  file: OpenFileTab;
}

export function UnsupportedFilePane({ file }: UnsupportedFilePaneProps) {
  const { t } = useTranslation("editor");
  return (
    <div className={styles.root}>
      <File size={32} className={styles.icon} />
      <p className={styles.fileName}>{file.fileName}</p>
      <p className={styles.path}>{file.relativePath}</p>
      <p className={styles.message}>{t("workspace.unsupportedFile.message")}</p>
    </div>
  );
}
