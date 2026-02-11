/**
 * Calculate string similarity using word overlap and containment scoring
 * Returns a value between 0 and 1
 */
export function similarity(a: string, b: string): number {
  const aLower = a.toLowerCase().trim();
  const bLower = b.toLowerCase().trim();

  if (aLower === bLower) return 1.0;
  if (aLower.length === 0 || bLower.length === 0) return 0;

  let score = 0;

  // Containment check with length-ratio penalty
  // Prevents short queries like "Air" from matching "Air Gear" too strongly
  const shorter = aLower.length <= bLower.length ? aLower : bLower;
  const longer = aLower.length <= bLower.length ? bLower : aLower;

  if (longer.includes(shorter)) {
    const lengthRatio = shorter.length / longer.length;
    score = Math.max(score, 0.8 * lengthRatio);
  }

  // Word overlap scoring (Jaccard similarity)
  const aWords = new Set(aLower.split(/\s+/));
  const bWords = new Set(bLower.split(/\s+/));
  const intersection = [...aWords].filter((w) => bWords.has(w));
  const union = new Set([...aWords, ...bWords]);

  if (union.size > 0) {
    score = Math.max(score, intersection.length / union.size);
  }

  return score;
}
