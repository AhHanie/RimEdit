/** Template written when creating a new Defs XML file from the project explorer. */
export const DEFS_FILE_TEMPLATE = '<?xml version="1.0" encoding="utf-8"?>\n<Defs>\n</Defs>\n';

/** Template written when creating a new Patches XML file from the project explorer. */
export const PATCH_FILE_TEMPLATE = '<?xml version="1.0" encoding="utf-8"?>\n<Patch>\n</Patch>\n';

/**
 * Returns `${baseName}.${extension}`, or that name suffixed with the lowest
 * available integer (`${baseName}1.${extension}`, `${baseName}2.${extension}`, ...)
 * if it collides case-insensitively with an existing sibling name.
 */
export function nextAvailableFileName(
  baseName: string,
  extension: string,
  siblingNames: string[],
): string {
  const taken = new Set(siblingNames.map((name) => name.toLowerCase()));
  const first = `${baseName}.${extension}`;
  if (!taken.has(first.toLowerCase())) return first;

  let suffix = 1;
  let candidate = `${baseName}${suffix}.${extension}`;
  while (taken.has(candidate.toLowerCase())) {
    suffix += 1;
    candidate = `${baseName}${suffix}.${extension}`;
  }
  return candidate;
}

/**
 * Ensures `name` ends with a (case-insensitive) `.xml` extension, appending one
 * if it's missing. Used so that retyping the suggested Defs/Patches file name
 * can't accidentally drop the extension the routing to the XML/patch editor
 * depends on.
 */
export function ensureXmlExtension(name: string): string {
  return /\.xml$/i.test(name) ? name : `${name}.xml`;
}
