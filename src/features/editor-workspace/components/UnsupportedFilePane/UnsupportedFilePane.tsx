import { File } from "lucide-react";
import type { OpenFileTab } from "../../types";
import styles from "./UnsupportedFilePane.module.css";

interface UnsupportedFilePaneProps {
  file: OpenFileTab;
}

export function UnsupportedFilePane({ file }: UnsupportedFilePaneProps) {
  return (
    <div className={styles.root}>
      <File size={32} className={styles.icon} />
      <p className={styles.fileName}>{file.fileName}</p>
      <p className={styles.path}>{file.relativePath}</p>
      <p className={styles.message}>
        Only XML files can be edited. Open this file in an external editor.
      </p>
    </div>
  );
}
