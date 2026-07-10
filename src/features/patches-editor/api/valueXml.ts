import { invoke } from "@tauri-apps/api/core";
import type { XmlChildView, XmlInitialElement } from "../../xml-editor";

/** Parse a patch operation's raw `<value>` inner XML into shape-classified child views, one per
 * top-level element (see `patches::value_xml::parse_value_fragment`). Stateless -- the value
 * fragment is never a real file, just a string held in the patch operation AST. */
export function parsePatchValueXml(valueXml: string): Promise<XmlChildView[]> {
  return invoke("parse_patch_value_xml", { valueXml });
}

/** Serialize a structured value edit back into XML text for a patch operation's `<value>`
 * payload. Reuses the same `XmlInitialElement` tree shape `xml-editor`'s object-list item
 * insertion already sends over IPC. */
export function serializePatchValueFragment(elements: XmlInitialElement[]): Promise<string> {
  return invoke("serialize_patch_value_fragment", { elements });
}
