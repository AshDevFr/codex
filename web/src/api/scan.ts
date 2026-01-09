import type { ScanProgress } from "@/types/api";
import { api } from "./client";

/**
 * SSE Reconnection Manager with exponential backoff
 */
class SSEReconnectionManager {
	private reconnectAttempts = 0;
	private maxAttempts = 10;
	private baseDelay = 1000;
	private maxDelay = 30000;
	private reconnectTimer: NodeJS.Timeout | null = null;
	private active = true;
	private url: string;
	private onMessage: (data: ScanProgress) => void;
	private onError?: (error: Error) => void;
	private onConnectionStateChange?: (
		state: "connecting" | "connected" | "disconnected" | "failed",
	) => void;

	constructor(
		url: string,
		onMessage: (data: ScanProgress) => void,
		onError?: (error: Error) => void,
		onConnectionStateChange?: (
			state: "connecting" | "connected" | "disconnected" | "failed",
		) => void,
	) {
		this.url = url;
		this.onMessage = onMessage;
		this.onError = onError;
		this.onConnectionStateChange = onConnectionStateChange;
	}

	private currentReader: ReadableStreamDefaultReader<Uint8Array> | null = null;

	async connect(): Promise<() => void> {
		this.onConnectionStateChange?.("connecting");

		const attemptConnection = async (): Promise<void> => {
			if (!this.active) return;

			try {
				const token = localStorage.getItem("jwt_token");
				if (!token) {
					// Not authenticated - silently skip connection
					return;
				}

				const response = await fetch(this.url, {
					headers: {
						Authorization: `Bearer ${token}`,
						Accept: "text/event-stream",
					},
				});

				if (!response.ok) {
					// Suppress 401 errors as they're expected when not authenticated
					if (response.status === 401) {
						return;
					}
					throw new Error(
						`SSE connection failed: ${response.status} ${response.statusText}`,
					);
				}

				if (!response.body) {
					throw new Error("Response body is null");
				}

				// Reset reconnection counter on successful connection
				this.reconnectAttempts = 0;
				this.onConnectionStateChange?.("connected");

				const reader = response.body.getReader();
				this.currentReader = reader;
				const decoder = new TextDecoder();
				let buffer = "";

				while (this.active) {
					const { done, value } = await reader.read();
					if (done) break;

					buffer += decoder.decode(value, { stream: true });
					const lines = buffer.split("\n\n");
					buffer = lines.pop() || "";

					for (const line of lines) {
						if (line.startsWith("data: ")) {
							try {
								const data = line.substring(6);
								if (data === "keep-alive") continue;
								const progress: ScanProgress = JSON.parse(data);
								this.onMessage(progress);
							} catch (error) {
								console.error("Failed to parse SSE data:", error);
							}
						}
					}
				}

				// Stream ended normally
				await reader.cancel();
				this.currentReader = null;
			} catch (error) {
				this.currentReader = null;
				if (!this.active) return;

				// Suppress "Not authenticated" errors as they're expected
				if (error instanceof Error && error.message === "Not authenticated") {
					return;
				}

				console.error("SSE connection error:", error);
				this.onConnectionStateChange?.("disconnected");

				// Call onError callback with the error
				if (error instanceof Error) {
					this.onError?.(error);
				}

				// Attempt reconnection with exponential backoff
				if (this.reconnectAttempts < this.maxAttempts) {
					const delay = Math.min(
						this.baseDelay * 2 ** this.reconnectAttempts,
						this.maxDelay,
					);
					this.reconnectAttempts++;

					console.debug(
						`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxAttempts})`,
					);

					this.reconnectTimer = setTimeout(() => {
						attemptConnection();
					}, delay);
				} else {
					this.onConnectionStateChange?.("failed");
				}
			}
		};

		attemptConnection();

		// Return cleanup function
		return () => {
			this.active = false;
			if (this.reconnectTimer) {
				clearTimeout(this.reconnectTimer);
			}
			if (this.currentReader) {
				this.currentReader.cancel();
				this.currentReader = null;
			}
		};
	}
}

export const scanApi = {
	/**
	 * Subscribe to scan progress updates via SSE with automatic reconnection
	 * Uses fetch with ReadableStream for proper authentication header support
	 */
	subscribeToProgress: (
		onProgress: (progress: ScanProgress) => void,
		onError?: (error: Error) => void,
		onConnectionStateChange?: (
			state: "connecting" | "connected" | "disconnected" | "failed",
		) => void,
	): (() => void) => {
		const baseURL = api.defaults.baseURL || "/api/v1";
		const manager = new SSEReconnectionManager(
			`${baseURL}/scans/stream`,
			onProgress,
			onError,
			onConnectionStateChange,
		);

		let cleanup: (() => void) | null = null;
		let isCleanedUp = false;

		manager.connect().then((cleanupFn) => {
			cleanup = cleanupFn;
			// If unsubscribe was called before cleanup was ready, call it now
			if (isCleanedUp) {
				cleanupFn();
			}
		});

		return () => {
			isCleanedUp = true;
			cleanup?.();
		};
	},
};
