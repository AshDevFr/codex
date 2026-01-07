import type { TaskProgressEvent } from "@/types/events";

interface TaskProgressReconnectionManager {
  connect: () => Promise<() => void>;
  disconnect: () => void;
}

/**
 * Create a reconnection manager for task progress SSE stream
 */
function createTaskProgressReconnectionManager(
  onEvent: (event: TaskProgressEvent) => void,
  onError?: (error: Error) => void,
  onConnectionStateChange?: (state: "connecting" | "connected" | "disconnected" | "failed") => void
): TaskProgressReconnectionManager {
  let reconnectAttempts = 0;
  const maxAttempts = 10;
  const baseDelay = 1000;
  const maxDelay = 30000;
  let currentAbortController: AbortController | null = null;
  let currentReader: ReadableStreamDefaultReader<Uint8Array> | null = null;
  let isActive = true;
  let reconnectTimeout: NodeJS.Timeout | null = null;

  const calculateDelay = (): number => {
    const delay = Math.min(baseDelay * Math.pow(2, reconnectAttempts), maxDelay);
    return delay;
  };

  const cleanup = () => {
    if (currentAbortController) {
      currentAbortController.abort();
      currentAbortController = null;
    }
    if (currentReader) {
      const cancelPromise = currentReader.cancel();
      if (cancelPromise && typeof cancelPromise.catch === 'function') {
        cancelPromise.catch(() => {});
      }
      currentReader = null;
    }
    if (reconnectTimeout) {
      clearTimeout(reconnectTimeout);
      reconnectTimeout = null;
    }
  };

  const connect = async (): Promise<void> => {
    if (!isActive) return;

    cleanup();

    const token = localStorage.getItem("jwt_token");
    if (!token) {
      onConnectionStateChange?.("failed");
      onError?.(new Error("No authentication token found"));
      return;
    }

    try {
      onConnectionStateChange?.("connecting");
      currentAbortController = new AbortController();

      const response = await fetch("/api/v1/tasks/stream", {
        headers: {
          Accept: "text/event-stream",
          Authorization: `Bearer ${token}`,
        },
        signal: currentAbortController.signal,
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      if (!response.body) {
        throw new Error("No response body");
      }

      onConnectionStateChange?.("connected");
      reconnectAttempts = 0; // Reset on successful connection

      currentReader = response.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";

      while (isActive) {
        const { done, value } = await currentReader.read();

        if (done) {
          break;
        }

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          if (line.startsWith("data: ")) {
            const data = line.substring(6);
            if (data === "keep-alive") continue;

            try {
              const event: TaskProgressEvent = JSON.parse(data);
              onEvent(event);
            } catch (e) {
              console.error("Failed to parse task progress event:", e);
            }
          }
        }
      }

      // Connection closed normally
      if (isActive) {
        scheduleReconnect();
      }
    } catch (error: unknown) {
      if (!isActive) return;

      // Ignore abort errors
      if (error instanceof Error && error.name === "AbortError") {
        return;
      }

      console.error("Task progress stream error:", error);
      onConnectionStateChange?.("disconnected");

      scheduleReconnect();
    }
  };

  const scheduleReconnect = () => {
    if (!isActive || reconnectAttempts >= maxAttempts) {
      if (reconnectAttempts >= maxAttempts) {
        onConnectionStateChange?.("failed");
        onError?.(new Error(`Max reconnection attempts (${maxAttempts}) reached`));
      }
      return;
    }

    reconnectAttempts++;
    const delay = calculateDelay();
    console.debug(
      `Reconnecting to task progress stream in ${delay}ms (attempt ${reconnectAttempts}/${maxAttempts})...`
    );

    reconnectTimeout = setTimeout(() => {
      connect();
    }, delay);
  };

  const disconnect = () => {
    isActive = false;
    cleanup();
    onConnectionStateChange?.("disconnected");
  };

  return {
    connect: async () => {
      await connect();
      return disconnect;
    },
    disconnect,
  };
}

/**
 * Subscribe to task progress events via SSE
 *
 * This creates a persistent connection to receive real-time updates about
 * background task execution (analyze_book, generate_thumbnails, etc.).
 *
 * Features:
 * - Automatic reconnection with exponential backoff
 * - Connection state tracking
 * - Authentication via JWT token
 *
 * @param onEvent - Callback for task progress events
 * @param onError - Optional callback for errors
 * @param onConnectionStateChange - Optional callback for connection state changes
 * @returns Cleanup function to close the connection
 */
export const subscribeToTaskProgress = (
  onEvent: (event: TaskProgressEvent) => void,
  onError?: (error: Error) => void,
  onConnectionStateChange?: (state: "connecting" | "connected" | "disconnected" | "failed") => void
): (() => void) => {
  const manager = createTaskProgressReconnectionManager(onEvent, onError, onConnectionStateChange);

  manager.connect();

  return () => {
    manager.disconnect();
  };
};
