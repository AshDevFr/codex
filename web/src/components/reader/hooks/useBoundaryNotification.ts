import { useCallback, useRef, useState } from "react";
import { useReaderStore } from "@/store/readerStore";

/**
 * Hook that manages the boundary notification message and auto-hide timeout.
 *
 * When a boundary event fires, the notification is shown for 3 seconds.
 * After the timeout, both the notification message and the store's boundary
 * state are cleared, so the next key press re-shows the overlay instead of
 * immediately changing volume.
 *
 * Each new boundary event cancels any pending timeout, preventing stale
 * timeouts from clearing the notification prematurely.
 *
 * The returned `clearNotification` function should be called when navigating
 * away from a boundary (e.g. going to a non-boundary page) to immediately
 * cancel any pending timeout and clear the notification.
 */
export function useBoundaryNotification() {
  const [message, setMessage] = useState<string | null>(null);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  const cancelTimeout = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, []);

  const clearNotification = useCallback(() => {
    cancelTimeout();
    setMessage(null);
    useReaderStore.getState().clearBoundaryState();
  }, [cancelTimeout]);

  const onBoundaryChange = useCallback(
    (_state: "none" | "at-start" | "at-end", msg: string | null) => {
      cancelTimeout();
      setMessage(msg);

      if (msg) {
        timeoutRef.current = setTimeout(() => {
          setMessage(null);
          useReaderStore.getState().clearBoundaryState();
          timeoutRef.current = null;
        }, 3000);
      }
    },
    [cancelTimeout],
  );

  return { message, onBoundaryChange, clearNotification };
}
