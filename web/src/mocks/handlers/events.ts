/**
 * SSE Events API mock handlers
 *
 * Provides mock handlers for Server-Sent Events endpoints.
 * These return simple keep-alive responses to prevent connection errors.
 */

import { HttpResponse, http } from "msw";

export const eventHandlers = [
	// Entity events SSE stream
	http.get("/api/v1/events/stream", () => {
		// Return an empty SSE response with keep-alive
		// MSW doesn't fully support streaming, so we return a minimal valid response
		const encoder = new TextEncoder();
		const stream = new ReadableStream({
			start(controller) {
				// Send initial keep-alive
				controller.enqueue(encoder.encode("data: keep-alive\n\n"));
				// Close the stream after initial response
				// In a real scenario, this would stay open
				controller.close();
			},
		});

		return new HttpResponse(stream, {
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
		const encoder = new TextEncoder();
		const stream = new ReadableStream({
			start(controller) {
				// Send initial keep-alive
				controller.enqueue(encoder.encode("data: keep-alive\n\n"));
				controller.close();
			},
		});

		return new HttpResponse(stream, {
			status: 200,
			headers: {
				"Content-Type": "text/event-stream",
				"Cache-Control": "no-cache",
				Connection: "keep-alive",
			},
		});
	}),
];
