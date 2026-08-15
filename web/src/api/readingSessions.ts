/**
 * Client for the batched reading-session endpoint.
 *
 * Sessions are the record of *what was read and for how long*; the progress
 * routes remain the record of *where the reader is*. Both are written, because
 * progress must stay correct for clients and API consumers that know nothing
 * about sessions.
 *
 * Delivery is best-effort by design. A dropped session costs some statistics;
 * it must never cost the reader their place or interrupt them with an error,
 * so nothing here throws into a reader.
 */

import { enqueueOfflineWrite, isOfflineError } from "@/lib/offline/outbox";
import type { ReadingSessionPayload } from "@/lib/reading/ReadingSessionTracker";
import { api } from "./client";

const API_BASE = "/api/v1";
const SESSIONS_PATH = `${API_BASE}/reading-sessions`;

export interface RecordSessionsResponse {
  accepted: string[];
  rejected: { id: string; reason: string }[];
}

function captureWriteHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  const token =
    typeof localStorage !== "undefined"
      ? localStorage.getItem("jwt_token")
      : null;
  if (token) headers.Authorization = `Bearer ${token}`;
  return headers;
}

export const readingSessionsApi = {
  /**
   * Send a batch of sessions.
   *
   * On network failure the batch goes to the offline outbox and is replayed
   * later. That replay is safe because each session carries a client-generated
   * id and the endpoint treats a repeat as a no-op, so a batch delivered twice
   * does not count the reading twice.
   */
  record: async (
    sessions: ReadingSessionPayload[],
  ): Promise<RecordSessionsResponse | null> => {
    if (sessions.length === 0) return null;

    try {
      const response = await api.post<RecordSessionsResponse>(
        "/reading-sessions",
        { sessions },
      );
      return response.data;
    } catch (err) {
      if (!isOfflineError(err)) throw err;
      await enqueueOfflineWrite({
        url: SESSIONS_PATH,
        method: "POST",
        headers: captureWriteHeaders(),
        body: { sessions },
      });
      return null;
    }
  },

  /**
   * Send a batch during page teardown.
   *
   * `sendBeacon` is the only transport the browser guarantees to complete once
   * the page is going away; a normal fetch is cancelled with the document.
   * Falls back to a keepalive fetch, then to the outbox, so a session is only
   * lost if all three fail.
   *
   * Returns whether the handoff was accepted, not whether the server stored it.
   */
  recordOnUnload: (sessions: ReadingSessionPayload[]): boolean => {
    if (sessions.length === 0) return true;

    const body = JSON.stringify({ sessions });

    if (typeof navigator !== "undefined" && navigator.sendBeacon) {
      // Beacons cannot carry an Authorization header, so this relies on the
      // session cookie. When the deployment is bearer-token only the beacon
      // is rejected and the keepalive fetch below is what actually delivers.
      const blob = new Blob([body], { type: "application/json" });
      if (navigator.sendBeacon(SESSIONS_PATH, blob)) return true;
    }

    try {
      void fetch(SESSIONS_PATH, {
        method: "POST",
        headers: captureWriteHeaders(),
        body,
        credentials: "include",
        keepalive: true,
      }).catch(() => {
        // Teardown is not a place to handle errors.
      });
      return true;
    } catch {
      void enqueueOfflineWrite({
        url: SESSIONS_PATH,
        method: "POST",
        headers: captureWriteHeaders(),
        body: { sessions },
      }).catch(() => {});
      return false;
    }
  },
};
