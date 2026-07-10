export interface UserDefTemplate {
  id: string;
  defType: string;
  name: string;
  description: string | null;
  xml: string;
  originalDefName: string | null;
  originalLabel: string | null;
  sourceRelativePath: string | null;
  gameVersion: string | null;
  createdAt: string;
  updatedAt: string;
}

// Listing projection that omits the full template `xml`.
export interface UserDefTemplateSummary {
  id: string;
  defType: string;
  name: string;
  description: string | null;
  originalDefName: string | null;
  originalLabel: string | null;
  sourceRelativePath: string | null;
  gameVersion: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface DeleteUserDefTemplateResult {
  deletedId: string;
}
