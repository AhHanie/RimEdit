import type { DiagnosticArgs } from "../../../lib/diagnostics";

/** Identifies which specialized editor/validation path a parsed document should use. */
export type XmlDocumentProfile = "defs" | "patch" | "about" | "genericXml";

export interface XmlDocumentLoadResult {
  projectId: string;
  relativePath: string;
  rawXml: string;
  document: XmlDocumentView | null;
  parseDiagnostics: ParseDiagnostic[];
  validationDiagnostics: ValidationDiagnostic[];
}

export interface XmlDocumentView {
  nodeCount: number;
  rootElement: string | null;
  profile: XmlDocumentProfile;
  defs: DefSummary[];
}

export interface DefSummary {
  nodeId: number;
  defType: string;
  defName: string | null;
  label: string | null;
  parentName: string | null;
  /** XML `Name` attribute - template identifier for abstract/inherited nodes. Not a def-database identity. */
  xmlName?: string | null;
  line: number | null;
  column: number | null;
}

export interface ParseDiagnostic {
  relativePath: string;
  line: number | null;
  column: number | null;
  byteOffset: number | null;
  message: string;
  code: string;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}

export type DiagnosticSeverity = "Error" | "Warning" | "Info";

export interface ValidationDiagnostic {
  relativePath: string;
  nodeId: number | null;
  line: number | null;
  column: number | null;
  severity: DiagnosticSeverity;
  message: string;
  code: string;
  defType: string | null;
  defName: string | null;
  fieldPath: string | null;
  blocking: boolean;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}

export type XmlEdit =
  | {
      type: "setChildElementText";
      parentNodeId: number;
      childName: string;
      value: string;
    }
  | {
      type: "setElementAttribute";
      elementNodeId: number;
      attributeName: string;
      value: string;
    }
  | {
      type: "removeChildElement";
      parentNodeId: number;
      childName: string;
    }
  | {
      type: "setListItems";
      parentNodeId: number;
      childName: string;
      items: string[];
    }
  | {
      type: "setNestedObjectFieldText";
      parentNodeId: number;
      objectName: string;
      fieldName: string;
      value: string;
    }
  | {
      type: "setNestedElementText";
      parentNodeId: number;
      objectPath: string[];
      fieldName: string;
      value: string;
      fieldOrder?: string[];
    }
  | {
      type: "setNestedListItems";
      parentNodeId: number;
      objectPath: string[];
      fieldName: string;
      items: string[];
      fieldOrder?: string[];
    }
  | {
      type: "setNamedMapEntry";
      parentNodeId: number;
      objectPath: string[];
      mapName: string;
      key: string;
      value: string;
      fieldOrder?: string[];
    }
  | {
      type: "removeNamedMapEntry";
      parentNodeId: number;
      objectPath: string[];
      mapName: string;
      key: string;
    }
  | {
      type: "renameNamedMapEntry";
      parentNodeId: number;
      objectPath: string[];
      mapName: string;
      oldKey: string;
      newKey: string;
      fieldOrder?: string[];
    }
  | {
      type: "setObjectListItemAttribute";
      listItemNodeId: number;
      attributeName: string;
      value: string;
    }
  | {
      type: "setObjectListItemChildText";
      listItemNodeId: number;
      childName: string;
      value: string;
      fieldOrder?: string[];
    }
  | {
      type: "removeObjectListItemChild";
      listItemNodeId: number;
      childName: string;
    }
  | {
      type: "insertObjectListItem";
      parentNodeId: number;
      /** Object path navigated from parentNodeId to reach the list container's parent. Empty for top-level lists. */
      objectPath?: string[];
      listName: string;
      classAttribute?: string;
      afterItemNodeId?: number;
      /** Field values to set on the new `<li>` in the same operation (flat scalars only). */
      initialChildFields?: { name: string; value: string }[];
      /** Field order used when creating child elements. */
      fieldOrder?: string[];
      /** Recursive initial element tree; supersedes initialChildFields when present. */
      initialChildren?: XmlInitialElement[];
    }
  | {
      type: "removeObjectListItem";
      listItemNodeId: number;
      /** When true, empty list container and ancestor object elements are pruned after removal. */
      pruneEmptyAncestors?: boolean;
    }
  | {
      type: "setTypedReferenceListItems";
      parentNodeId: number;
      objectPath: string[];
      fieldName: string;
      items: { defType: string; defName: string }[];
    }
  | {
      type: "removeElementAttribute";
      elementNodeId: number;
      attributeName: string;
    }
  | {
      type: "removeNestedElement";
      parentNodeId: number;
      objectPath: string[];
      fieldName: string;
      pruneEmptyAncestors?: boolean;
    }
  | {
      type: "replaceKeyedValueListEntries";
      parentNodeId: number;
      objectPath: string[];
      mapName: string;
      entries: { key: string; value: string }[];
    }
  | {
      type: "insertKeyedObjectListItem";
      parentNodeId: number;
      /** Object path navigated from parentNodeId to reach the container's parent. Empty for top-level containers. */
      objectPath?: string[];
      listName: string;
      /** Element name of the new item (the key / def name). */
      keyName: string;
      afterItemNodeId?: number;
      /** Field order used when creating the list container. */
      fieldOrder?: string[];
      /** Recursive initial element tree for the new item's fields. */
      initialChildren?: XmlInitialElement[];
    }
  | {
      type: "renameKeyedObjectListItem";
      /** Node id of the keyed element to rename. */
      itemNodeId: number;
      /** New element name (new key / def name). */
      newName: string;
    }
  | {
      type: "setKeyedObjectListItemText";
      /** Node id of the keyed item element (e.g. `<Corpse>`). */
      itemNodeId: number;
      /** New scalar text value for the item element. */
      value: string;
    }
  | {
      type: "setNestedElementAttribute";
      parentNodeId: number;
      objectPath: string[];
      attributeName: string;
      value: string;
    };

/**
 * Recursive initial element tree used when inserting a new object-list item.
 * Each node represents one XML element to create under its parent.
 * - `value`: text content for scalar elements
 * - `attributes`: attributes to set (e.g. Class discriminator)
 * - `children`: named child elements (object fields, namedMap entries, typedReferenceList)
 * - `liItems`: `<li>` children (listOfLi fields, nested object lists)
 */
export interface XmlInitialElement {
  name: string;
  value?: string;
  attributes?: { name: string; value: string }[];
  children?: XmlInitialElement[];
  liItems?: XmlInitialElement[];
}

export interface XmlEditContext {
  fieldOrder: string[];
  nestedFieldOrders?: Record<string, string[]>;
}

export type XmlChildShape = "element" | "object" | "listOfLi" | "namedChildrenMap" | "keyedValueList";

export interface XmlAttributeView {
  name: string;
  value: string;
  known: boolean;
}

/** Rich per-`<li>` view for object-list fields. Preserves attributes, children, and self-closing state. */
export interface XmlListItemView {
  nodeId: number;
  textValue: string | null;
  attributes: XmlAttributeView[];
  children: XmlNestedChildView[];
  order: number;
  line: number | null;
  column: number | null;
  selfClosing: boolean;
}

export interface XmlNestedChildView {
  nodeId: number;
  name: string;
  textValue: string | null;
  listItems: string[];
  xmlShape: XmlChildShape;
  children?: XmlNestedChildView[];
  order: number;
  line: number | null;
  column: number | null;
  /** Attributes on this element. */
  attributes?: XmlAttributeView[];
  /** For listOfLi shape: element children of each `<li>` item; empty inner array for scalar items. */
  liObjectItems?: XmlNestedChildView[][];
  /** Rich per-`<li>` view for object-list fields with attributes and full item state. */
  liItems?: XmlListItemView[];
}

export interface XmlChildView {
  nodeId: number;
  name: string;
  textValue: string | null;
  listItems: string[];
  xmlShape: XmlChildShape;
  children?: XmlNestedChildView[];
  order: number;
  known: boolean;
  line: number | null;
  column: number | null;
  /** Attributes on this element (e.g. `Class` discriminator on object-typed children). */
  attributes?: XmlAttributeView[];
  /** For listOfLi shape: element children of each `<li>` item; empty inner array for scalar items. */
  liObjectItems?: XmlNestedChildView[][];
  /** Rich per-`<li>` view for object-list fields with attributes and full item state. */
  liItems?: XmlListItemView[];
}

export interface DefEditorView extends DefSummary {
  attributes: XmlAttributeView[];
  children: XmlChildView[];
}

export interface XmlEditorDocumentView {
  nodeCount: number;
  rootElement: string | null;
  profile: XmlDocumentProfile;
  defs: DefEditorView[];
  about: AboutMetadataView | null;
}

export interface XmlEditorDocumentLoadResult {
  projectId: string;
  relativePath: string;
  rawXml: string;
  document: XmlEditorDocumentView | null;
  parseDiagnostics: ParseDiagnostic[];
  validationDiagnostics: ValidationDiagnostic[];
}

// --- About.xml (ModMetaData) metadata view ---------------------------------

export interface AboutScalarField {
  value: string | null;
}

export interface AboutListField {
  items: string[];
  /** True when the container element exists in the XML, even with no `<li>` children. */
  present: boolean;
}

export interface AboutDependency {
  nodeId: number;
  packageId: string | null;
  alternativePackageIds: string[];
  displayName: string | null;
  downloadUrl: string | null;
  steamWorkshopUrl: string | null;
}

export interface AboutVersionedTextEntry {
  /** Raw XML element name used as the version key (e.g. `"v1.6"`). */
  version: string;
  value: string;
}

export interface AboutVersionedListEntry {
  version: string;
  items: string[];
}

export interface AboutVersionedDependenciesEntry {
  version: string;
  dependencies: AboutDependency[];
}

export interface AboutMetadataFields {
  packageId: AboutScalarField;
  name: AboutScalarField;
  shortName: AboutScalarField;
  author: AboutScalarField;
  authors: AboutListField;
  modIconPath: AboutScalarField;
  modVersion: AboutScalarField;
  url: AboutScalarField;
  description: AboutScalarField;
  steamAppId: AboutScalarField;
  /** Obsolete field; kept only so the UI can show a warning and offer removal. */
  targetVersion: AboutScalarField;
  supportedVersions: AboutListField;
  loadBefore: AboutListField;
  loadAfter: AboutListField;
  forceLoadBefore: AboutListField;
  forceLoadAfter: AboutListField;
  incompatibleWith: AboutListField;
  modDependencies: AboutDependency[];
  descriptionsByVersion: AboutVersionedTextEntry[];
  modDependenciesByVersion: AboutVersionedDependenciesEntry[];
  loadBeforeByVersion: AboutVersionedListEntry[];
  loadAfterByVersion: AboutVersionedListEntry[];
  incompatibleWithByVersion: AboutVersionedListEntry[];
}

export interface AboutUnknownElement {
  nodeId: number;
  name: string;
  line: number | null;
  column: number | null;
}

export interface AboutMetadataView {
  rootNodeId: number;
  fields: AboutMetadataFields;
  unknownChildren: AboutUnknownElement[];
}
