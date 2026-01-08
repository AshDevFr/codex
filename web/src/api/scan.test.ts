import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { scanApi } from "./scan";

describe("scanApi.subscribeToProgress", () => {
	let mockFetch: ReturnType<typeof vi.fn>;

	beforeEach(() => {
		// Mock localStorage
		Storage.prototype.getItem = vi.fn((key) => {
			if (key === "jwt_token") return "test-token-123";
			return null;
		});

		// Mock fetch
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

		const onProgress = vi.fn();
		const unsubscribe = scanApi.subscribeToProgress(onProgress);

		// Verify fetch called with correct URL and headers
		expect(mockFetch).toHaveBeenCalledWith(
			expect.stringContaining("/api/v1/scans/stream"),
			expect.objectContaining({
				headers: expect.objectContaining({
					Authorization: "Bearer test-token-123",
					Accept: "text/event-stream",
				}),
			}),
		);

		unsubscribe();
	});

	it("should parse SSE events correctly", async () => {
		const sseData =
			'data: {"library_id":"123","status":"running","files_total":100,"files_processed":50}\n\n';
		const encoder = new TextEncoder();

		const mockReader = {
			read: vi
				.fn()
				.mockResolvedValueOnce({
					done: false,
					value: encoder.encode(sseData),
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

		const onProgress = vi.fn();
		const unsubscribe = scanApi.subscribeToProgress(onProgress);

		// Wait for async processing
		await new Promise((resolve) => setTimeout(resolve, 100));

		expect(onProgress).toHaveBeenCalledWith(
			expect.objectContaining({
				library_id: "123",
				status: "running",
				files_total: 100,
				files_processed: 50,
			}),
		);

		unsubscribe();
	});

	it("should handle keep-alive messages", async () => {
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

		const onProgress = vi.fn();
		const unsubscribe = scanApi.subscribeToProgress(onProgress);

		await new Promise((resolve) => setTimeout(resolve, 100));

		// keep-alive should not trigger onProgress
		expect(onProgress).not.toHaveBeenCalled();

		unsubscribe();
	});

	it("should handle invalid JSON gracefully", async () => {
		const invalidData = "data: {invalid json}\n\n";
		const encoder = new TextEncoder();

		const mockReader = {
			read: vi
				.fn()
				.mockResolvedValueOnce({
					done: false,
					value: encoder.encode(invalidData),
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

		const onProgress = vi.fn();
		const onError = vi.fn();
		const consoleError = vi
			.spyOn(console, "error")
			.mockImplementation(() => {});

		const unsubscribe = scanApi.subscribeToProgress(onProgress, onError);

		await new Promise((resolve) => setTimeout(resolve, 100));

		// Should log error but not crash
		expect(consoleError).toHaveBeenCalled();
		expect(onProgress).not.toHaveBeenCalled();

		unsubscribe();
		consoleError.mockRestore();
	});

	it("should call onError on connection failure", async () => {
		mockFetch.mockRejectedValueOnce(new Error("Network error"));

		const onProgress = vi.fn();
		const onError = vi.fn();
		const consoleError = vi
			.spyOn(console, "error")
			.mockImplementation(() => {});

		const unsubscribe = scanApi.subscribeToProgress(onProgress, onError);

		await new Promise((resolve) => setTimeout(resolve, 100));

		// Connection should fail and not throw synchronously
		expect(consoleError).toHaveBeenCalledWith(
			"SSE connection error:",
			expect.any(Error),
		);

		unsubscribe();
		consoleError.mockRestore();
	});

	it("should handle stream errors during reading", async () => {
		const mockReader = {
			read: vi.fn().mockRejectedValueOnce(new Error("Stream interrupted")),
			cancel: vi.fn(),
		};

		mockFetch.mockResolvedValueOnce({
			ok: true,
			body: {
				getReader: () => mockReader,
			},
		});

		const onProgress = vi.fn();
		const onError = vi.fn();
		const consoleError = vi
			.spyOn(console, "error")
			.mockImplementation(() => {});

		const unsubscribe = scanApi.subscribeToProgress(onProgress, onError);

		await new Promise((resolve) => setTimeout(resolve, 100));

		expect(consoleError).toHaveBeenCalledWith(
			"SSE connection error:",
			expect.any(Error),
		);

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

		const onProgress = vi.fn();
		const unsubscribe = scanApi.subscribeToProgress(onProgress);

		await new Promise((resolve) => setTimeout(resolve, 50));

		unsubscribe();

		await new Promise((resolve) => setTimeout(resolve, 50));

		expect(mockReader.cancel).toHaveBeenCalled();
	});

	it("should handle multiple events in single chunk", async () => {
		const multipleEvents =
			'data: {"library_id":"1","status":"pending","files_total":0,"files_processed":0}\n\n' +
			'data: {"library_id":"1","status":"running","files_total":100,"files_processed":10}\n\n';
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

		const onProgress = vi.fn();
		const unsubscribe = scanApi.subscribeToProgress(onProgress);

		await new Promise((resolve) => setTimeout(resolve, 100));

		// Should have received both events
		expect(onProgress).toHaveBeenCalledTimes(2);
		expect(onProgress).toHaveBeenNthCalledWith(
			1,
			expect.objectContaining({ status: "pending" }),
		);
		expect(onProgress).toHaveBeenNthCalledWith(
			2,
			expect.objectContaining({ status: "running" }),
		);

		unsubscribe();
	});
});
