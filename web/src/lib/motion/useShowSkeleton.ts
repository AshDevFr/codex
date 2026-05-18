import { useDelayedFlag } from "./useDelayedFlag";

/**
 * Shared skeleton-flicker constant: only flip to "show skeleton" once a load
 * has been pending for this many milliseconds. Anything shorter looks like a
 * flash and reads worse than a blank pause.
 */
export const SKELETON_DELAY_MS = 150;

/**
 * Thin wrapper over `useDelayedFlag` that bakes in the 150ms gate every
 * page uses for skeleton loading states. Returns `true` once `isLoading`
 * has been true continuously for `SKELETON_DELAY_MS`, and falls back to
 * `false` immediately when loading completes.
 */
export function useShowSkeleton(isLoading: boolean): boolean {
  return useDelayedFlag(isLoading, SKELETON_DELAY_MS);
}
