import { useCallback, useEffect, useRef, useState } from "react";

/** Default countdown length, in seconds, before auto-advancing to the next book. */
export const DEFAULT_AUTO_ADVANCE_SECONDS = 10;

interface UseAutoAdvanceCountdownOptions {
  /**
   * Whether the countdown should run. Typically
   * `direction === "next" && autoAdvanceSetting && hasNextBook`.
   * When false, the hook is inert and `remaining` holds the full duration.
   */
  active: boolean;
  /** Countdown duration in seconds. Defaults to {@link DEFAULT_AUTO_ADVANCE_SECONDS}. */
  seconds?: number;
  /** Called exactly once when the countdown reaches zero (unless cancelled). */
  onElapsed: () => void;
}

interface UseAutoAdvanceCountdownResult {
  /** Seconds left on the countdown. */
  remaining: number;
  /** Stop the countdown; `onElapsed` will not fire. */
  cancel: () => void;
  /** Whether the user cancelled the countdown. */
  cancelled: boolean;
}

/**
 * Drives the auto-advance countdown shown on the "Next Chapter" transition
 * panel. Ticks `remaining` down once per second while `active`; when it hits
 * zero it calls `onElapsed` a single time. Cancelling halts the timer, and the
 * cancelled state resets once the countdown goes inactive so a later
 * re-activation starts fresh.
 */
export function useAutoAdvanceCountdown({
  active,
  seconds = DEFAULT_AUTO_ADVANCE_SECONDS,
  onElapsed,
}: UseAutoAdvanceCountdownOptions): UseAutoAdvanceCountdownResult {
  const [remaining, setRemaining] = useState(seconds);
  const [cancelled, setCancelled] = useState(false);

  // Keep the latest callback without restarting the interval each render.
  const onElapsedRef = useRef(onElapsed);
  onElapsedRef.current = onElapsed;

  const cancel = useCallback(() => setCancelled(true), []);

  // Reset when the countdown is not running so re-activation starts clean.
  useEffect(() => {
    if (!active) {
      setCancelled(false);
      setRemaining(seconds);
    }
  }, [active, seconds]);

  // Tick once per second while active and not cancelled.
  useEffect(() => {
    if (!active || cancelled) return;
    setRemaining(seconds);
    const interval = setInterval(() => {
      setRemaining((r) => Math.max(0, r - 1));
    }, 1000);
    return () => clearInterval(interval);
  }, [active, cancelled, seconds]);

  // Fire onElapsed exactly once when the countdown reaches zero.
  useEffect(() => {
    if (active && !cancelled && remaining === 0) {
      onElapsedRef.current();
    }
  }, [active, cancelled, remaining]);

  return { remaining, cancel, cancelled };
}
