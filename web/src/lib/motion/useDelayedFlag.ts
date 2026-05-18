import { useEffect, useState } from "react";

/**
 * Skeleton flicker guard. Returns `false` until `source` has stayed true
 * continuously for `delayMs`. Flips back to `false` immediately when the
 * source goes false again.
 *
 * Why: a skeleton that flashes for 50ms feels worse than no skeleton at
 * all. Pages use this to gate the swap between a real component and its
 * shape-matched skeleton, so fast loads stay flash-free while slow loads
 * still get a placeholder.
 */
export function useDelayedFlag(source: boolean, delayMs = 150): boolean {
  const [flag, setFlag] = useState(false);

  useEffect(() => {
    if (!source) {
      setFlag(false);
      return;
    }

    const timer = window.setTimeout(() => {
      setFlag(true);
    }, delayMs);

    return () => {
      window.clearTimeout(timer);
    };
  }, [source, delayMs]);

  return flag;
}
