export function replaceAt<T>(items: T[], index: number, value: T): T[] {
  return items.map((item, i) => (i === index ? value : item));
}

export function removeAt<T>(items: T[], index: number): T[] {
  return items.filter((_, i) => i !== index);
}

export function insertAt<T>(items: T[], index: number, value: T): T[] {
  const next = items.slice();
  next.splice(index, 0, value);
  return next;
}

/** Swaps `index` with its neighbor in `direction` (-1 = up, 1 = down). No-op if the neighbor is
 * out of bounds. */
export function moveItem<T>(items: T[], index: number, direction: -1 | 1): T[] {
  const target = index + direction;
  if (target < 0 || target >= items.length) return items;
  const next = items.slice();
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

/** `""` display state maps to `null` on commit -- new/cleared scalar fields are treated as
 * "field absent" rather than "field present but empty", matching how the backend parser
 * distinguishes a missing element from an empty one. */
export function emptyToNull(value: string): string | null {
  return value === "" ? null : value;
}

export function nullToEmpty(value: string | null | undefined): string {
  return value ?? "";
}
