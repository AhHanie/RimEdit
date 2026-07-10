import type { ValidationDiagnostic } from "../../xml-editor/types/xmlDocument";

export function diagnosticsForField(
  diagnostics: ValidationDiagnostic[],
  fieldPath: string,
): ValidationDiagnostic[] {
  return diagnostics.filter((d) => d.fieldPath === fieldPath);
}

export function diagnosticsForNode(
  diagnostics: ValidationDiagnostic[],
  nodeId: number,
): ValidationDiagnostic[] {
  return diagnostics.filter((d) => d.nodeId === nodeId);
}

export function fieldSeverity(diagnostics: ValidationDiagnostic[]): "Error" | "Warning" | null {
  if (diagnostics.some((d) => d.severity === "Error")) return "Error";
  if (diagnostics.some((d) => d.severity === "Warning")) return "Warning";
  return null;
}
