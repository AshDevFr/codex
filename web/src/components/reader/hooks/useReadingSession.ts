/**
 * Owns a {@link ReadingSessionTracker} for the reader that mounts it.
 *
 * The tracker itself is deliberately framework-free so the iOS client can
 * mirror it. This hook is the React-shaped half: it maps component lifecycle
 * and browser events onto the tracker's calls, and nothing more. Measurement
 * rules do not belong here.
 */

import { useCallback, useEffect, useRef } from "react";
import { readingSessionsApi } from "@/api/readingSessions";
import { getDeviceId, getDeviceName } from "@/lib/reading/deviceIdentity";
import {
  type ReadingPosition,
  type ReadingSessionPayload,
  ReadingSessionTracker,
} from "@/lib/reading/ReadingSessionTracker";

interface UseReadingSessionOptions {
  bookId: string;
  /** Set false to stop measuring, e.g. in incognito reading. */
  enabled?: boolean;
}

interface UseReadingSessionReturn {
  /** Report a page turn, scroll, pinch, or TOC jump. */
  recordActivity: (position: ReadingPosition) => void;
  /** Report that the book was finished. */
  markCompleted: (position?: ReadingPosition) => void;
  /** Report that the book was marked unread. */
  markReset: () => void;
}

function send(sessions: ReadingSessionPayload[]): void {
  // Never surfaced to the reader: losing a session costs statistics, and an
  // error toast mid-page-turn would cost far more.
  void readingSessionsApi.record(sessions).catch(() => {});
}

export function useReadingSession({
  bookId,
  enabled = true,
}: UseReadingSessionOptions): UseReadingSessionReturn {
  const trackerRef = useRef<ReadingSessionTracker | null>(null);

  useEffect(() => {
    if (!enabled || !bookId) {
      trackerRef.current = null;
      return;
    }

    const tracker = new ReadingSessionTracker({
      bookId,
      deviceId: getDeviceId(),
      deviceName: getDeviceName(),
      emit: send,
    });
    trackerRef.current = tracker;

    // Hidden covers backgrounding, tab switching, and screen lock on mobile,
    // which is every case where the reader is no longer being read.
    const onVisibility = () => {
      if (document.visibilityState === "hidden") {
        tracker.pause();
        tracker.checkpointNow();
      } else {
        tracker.resume();
      }
    };

    // `pagehide` rather than `beforeunload`: the latter does not fire reliably
    // on mobile Safari, which is exactly where tabs get killed most.
    const onPageHide = () => {
      tracker.checkpointNow();
      const pending = tracker.peek();
      if (pending) readingSessionsApi.recordOnUnload([pending]);
    };

    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("pagehide", onPageHide);

    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("pagehide", onPageHide);
      // Closing the reader ends the session; the tracker emits it.
      tracker.stop();
      trackerRef.current = null;
    };
  }, [bookId, enabled]);

  const recordActivity = useCallback((position: ReadingPosition) => {
    trackerRef.current?.recordActivity(position);
  }, []);

  const markCompleted = useCallback((position: ReadingPosition = {}) => {
    trackerRef.current?.markCompleted(position);
  }, []);

  const markReset = useCallback(() => {
    trackerRef.current?.markReset();
  }, []);

  return { recordActivity, markCompleted, markReset };
}
