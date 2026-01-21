/**
 * SSE Events API mock handlers
 *
 * Provides mock handlers for Server-Sent Events endpoints.
 * These return streams that stay open to prevent reconnection spam in tests.
 * The streams send periodic keep-alives but don't close immediately,
 * which prevents the SSE clients from triggering reconnection logic.
 */

import { HttpResponse, http } from "msw";

/**
 * Creates an SSE stream that stays open and sends periodic keep-alives.
 * The stream doesn't close automatically, preventing reconnection attempts.
 * Tests clean up via component unmounting which cancels the fetch.
 */
function createMockSseStream(): ReadableStream<Uint8Array> {
	const encoder = new TextEncoder();
	let intervalId: ReturnType<typeof setInterval> | null = null;

	return new ReadableStream({
		start(controller) {
			// Send initial keep-alive
			controller.enqueue(encoder.encode("data: keep-alive\n\n"));

			// Send periodic keep-alives to keep the connection "active"
			// This prevents the client from thinking the connection dropped
			intervalId = setInterval(() => {
				try {
					controller.enqueue(encoder.encode("data: keep-alive\n\n"));
				} catch {
					// Stream was cancelled, stop the interval
					if (intervalId) {
						clearInterval(intervalId);
						intervalId = null;
					}
				}
			}, 30000); // Every 30 seconds (won't actually fire in short tests)
		},
		cancel() {
			// Clean up interval when stream is cancelled
			if (intervalId) {
				clearInterval(intervalId);
				intervalId = null;
			}
		},
	});
}

export const eventHandlers = [
	// Entity events SSE stream
	http.get("/api/v1/events/stream", () => {
		return new HttpResponse(createMockSseStream(), {
			status: 200,
			headers: {
				"Content-Type": "text/event-stream",
				"Cache-Control": "no-cache",
				Connection: "keep-alive",
			},
		});
	}),

	// Task progress SSE stream
	http.get("/api/v1/tasks/stream", () => {
		return new HttpResponse(createMockSseStream(), {
			status: 200,
			headers: {
				"Content-Type": "text/event-stream",
				"Cache-Control": "no-cache",
				Connection: "keep-alive",
			},
		});
	}),
];
