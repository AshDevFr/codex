/**
 * Pair each item with a React key derived from its content.
 *
 * For display lists whose items have no unique id. Unlike an array index, a
 * content key stays with its item when other items are added or removed.
 * Repeated identities get an occurrence suffix (`a`, `a#2`, `a#3`) so every
 * key is unique even when the data repeats.
 *
 * @example
 * withContentKeys(series, (s) => s.path ?? s.name).map(({ key, item }) => (
 *   <SeriesCard key={key} series={item} />
 * ));
 */
export function withContentKeys<T>(
  items: readonly T[],
  identify: (item: T) => string,
): Array<{ key: string; item: T }> {
  const used = new Set<string>();
  const seen = new Map<string, number>();

  return items.map((item) => {
    const identity = identify(item);
    let occurrence = seen.get(identity) ?? 0;
    let key: string;
    do {
      occurrence += 1;
      key = occurrence === 1 ? identity : `${identity}#${occurrence}`;
    } while (used.has(key));
    seen.set(identity, occurrence);
    used.add(key);
    return { key, item };
  });
}
