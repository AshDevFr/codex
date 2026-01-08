import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { eventsApi } from "./events";

describe("eventsApi.subscribeToEntityEvents", () => {
	let mockFetch: ReturnType<typeof vi.fn>;

	beforeEach(() => {
		Storage.prototype.getItem = vi.fn((key) => {
			if (key === "jwt_token") return "test-token-456";
			return null;
		});

		mockFetch = vi.fn();
		global.fetch = mockFetch;
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("should connect with Authorization header", async () => {
		const mockReader = {
			read: vi.fn().mockResolvedValueOnce({ done: true, value: undefined }),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const unsubscribe = eventsApi.subscribeToEntityEvents(onEvent);

		expect(mockFetch).toHaveBeenCalledWith(
			expect.stringContaining("/api/v1/events/stream"),
			expect.objectContaining({
				headers: expect.objectContaining({
					Authorization: "Bearer test-token-456",
					Accept: "text/event-stream",
				}),
			}),
		);

		unsubscribe();
	});

	it("should parse CoverUpdated events correctly", async () => {
		const eventData =
			'data: {"event_type":{"CoverUpdated":{"entity_type":"Series","entity_id":"abc-123","library_id":"lib-1"}},"timestamp":"2026-01-07T12:00:00Z","user_id":"user-1"}\n\n';
		const encoder = new TextEncoder();

		const mockReader = {
			read: vi
				.fn()
				.mockResolvedValueOnce({
					done: false,
					value: encoder.encode(eventData),
				})
				.mockResolvedValueOnce({ done: true, value: undefined }),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const unsubscribe = eventsApi.subscribeToEntityEvents(onEvent);

		await new Promise((resolve) => setTimeout(resolve, 100));

		expect(onEvent).toHaveBeenCalledWith(
			expect.objectContaining({
				event_type: expect.objectContaining({
					CoverUpdated: expect.objectContaining({
						entity_type: "Series",
						entity_id: "abc-123",
					}),
				}),
				timestamp: "2026-01-07T12:00:00Z",
				user_id: "user-1",
			}),
		);

		unsubscribe();
	});

	it("should handle SeriesBulkPurged events", async () => {
		const eventData =
			'data: {"event_type":{"SeriesBulkPurged":{"series_id":"series-1","library_id":"lib-1","count":5}},"timestamp":"2026-01-07T12:00:00Z","user_id":"user-1"}\n\n';
		const encoder = new TextEncoder();

		const mockReader = {
			read: vi
				.fn()
				.mockResolvedValueOnce({
					done: false,
					value: encoder.encode(eventData),
				})
				.mockResolvedValueOnce({ done: true, value: undefined }),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const unsubscribe = eventsApi.subscribeToEntityEvents(onEvent);

		await new Promise((resolve) => setTimeout(resolve, 100));

		expect(onEvent).toHaveBeenCalledWith(
			expect.objectContaining({
				event_type: expect.objectContaining({
					SeriesBulkPurged: expect.objectContaining({
						series_id: "series-1",
						count: 5,
					}),
				}),
			}),
		);

		unsubscribe();
	});

	it("should handle keep-alive messages without triggering callback", async () => {
		const keepAlive = "data: keep-alive\n\n";
		const encoder = new TextEncoder();

		const mockReader = {
			read: vi
				.fn()
				.mockResolvedValueOnce({
					done: false,
					value: encoder.encode(keepAlive),
				})
				.mockResolvedValueOnce({ done: true, value: undefined }),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const unsubscribe = eventsApi.subscribeToEntityEvents(onEvent);

		await new Promise((resolve) => setTimeout(resolve, 100));

		expect(onEvent).not.toHaveBeenCalled();

		unsubscribe();
	});

	it("should handle connection state changes", async () => {
		const mockReader = {
			read: vi.fn().mockResolvedValueOnce({ done: true, value: undefined }),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const onConnectionChange = vi.fn();
		const unsubscribe = eventsApi.subscribeToEntityEvents(
			onEvent,
			undefined,
			onConnectionChange,
		);

		await new Promise((resolve) => setTimeout(resolve, 50));

		// Should report connecting first, then connected
		expect(onConnectionChange).toHaveBeenCalledWith("connecting");
		expect(onConnectionChange).toHaveBeenCalledWith("connected");

		unsubscribe();
	});

	it("should call onError on stream errors", async () => {
		const mockReader = {
			read: vi.fn().mockRejectedValueOnce(new Error("Connection lost")),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const onError = vi.fn();
		const consoleError = vi
			.spyOn(console, "error")
			.mockImplementation(() => {});

		const unsubscribe = eventsApi.subscribeToEntityEvents(onEvent, onError);

		await new Promise((resolve) => setTimeout(resolve, 100));

		expect(onError).toHaveBeenCalledWith(expect.any(Error));

		unsubscribe();
		consoleError.mockRestore();
	});

	it("should cleanup properly on unsubscribe", async () => {
		const mockReader = {
			read: vi
				.fn()
				.mockResolvedValueOnce({ done: false, value: new Uint8Array() })
				.mockResolvedValue({ done: true, value: undefined }),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const unsubscribe = eventsApi.subscribeToEntityEvents(onEvent);

		await new Promise((resolve) => setTimeout(resolve, 50));

		unsubscribe();

		await new Promise((resolve) => setTimeout(resolve, 50));

		expect(mockReader.cancel).toHaveBeenCalled();
	});

	it("should handle multiple events in sequence", async () => {
		const multipleEvents =
			'data: {"event_type":{"CoverUpdated":{"entity_type":"Series","entity_id":"1","library_id":"lib-1"}},"timestamp":"2026-01-07T12:00:00Z","user_id":null}\n\n' +
			'data: {"event_type":{"SeriesBulkPurged":{"series_id":"2","library_id":"lib-1","count":3}},"timestamp":"2026-01-07T12:01:00Z","user_id":"user-1"}\n\n';
		const encoder = new TextEncoder();

		const mockReader = {
			read: vi
				.fn()
				.mockResolvedValueOnce({
					done: false,
					value: encoder.encode(multipleEvents),
				})
				.mockResolvedValueOnce({ done: true, value: undefined }),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onEvent = vi.fn();
		const unsubscribe = eventsApi.subscribeToEntityEvents(onEvent);

		await new Promise((resolve) => setTimeout(resolve, 100));

		expect(onEvent).toHaveBeenCalledTimes(2);
		expect(onEvent).toHaveBeenNthCalledWith(
			1,
			expect.objectContaining({
				event_type: expect.objectContaining({
					CoverUpdated: expect.any(Object),
				}),
			}),
		);
		expect(onEvent).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({
				event_type: expect.objectContaining({
					SeriesBulkPurged: expect.any(Object),
				}),
			}),
		);

		unsubscribe();
	});
});
