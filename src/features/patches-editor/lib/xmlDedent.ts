/** Strips the common leading indentation and surrounding blank lines from a raw XML fragment.
 * Patch operation `valueXml` is captured verbatim from the source file's original span (see
 * `patches::parser::element_inner_xml` in the backend) so it round-trips byte-for-byte when
 * untouched -- but that means it carries whatever indentation the value happened to have in the
 * mod's file, which looks broken when shown in the Raw XML box. This is purely a display-time
 * transform; the untouched `valueXml` prop is never mutated by it. */
export function dedentXmlFragment(xml: string): string {
  const lines = xml.replace(/\r\n/g, "\n").split("\n");

  while (lines.length && lines[0].trim() === "") lines.shift();
  while (lines.length && lines[lines.length - 1].trim() === "") lines.pop();
  if (lines.length === 0) return "";

  const indents = lines.filter((line) => line.trim() !== "").map((line) => line.match(/^[ \t]*/)![0].length);
  const minIndent = indents.length ? Math.min(...indents) : 0;

  return lines.map((line) => (line.trim() === "" ? "" : line.slice(minIndent))).join("\n");
}
