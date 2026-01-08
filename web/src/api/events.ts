import type { EntityChangeEvent } from "@/types/events";

/**
 * SSE Reconnection Manager for entity events with exponential backoff
 */
class EntityEventsReconnectionManager {
	private reconnectAttempts = 0;
	private maxAttempts = 10;
	private baseDelay = 1000;
	private maxDelay = 30000;
	private reconnectTimer: NodeJS.Timeout | null = null;
	private active = true;
	private url: string;
	private onEvent: (event: EntityChangeEvent) => void;
	private onError?: (error: Error) => void;
	private onConnectionStateChange?: (
		state: "connecting" | "connected" | "disconnected" | "failed",
	) => void;

	constructor(
		url: string,
		onEvent: (event: EntityChangeEvent) => void,
		onError?: (error: Error) => void,
		onConnectionStateChange?: (
			state: "connecting" | "connected" | "disconnected" | "failed",
		) => void,
	) {
		this.url = url;
		this.onEvent = onEvent;
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
					throw new Error("Not authenticated");
				}

				const response = await fetch(this.url, {
					headers: {
						Authorization: `Bearer ${token}`,
						Accept: "text/event-stream",
					},
				});

				if (!response.ok) {
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
								const event: EntityChangeEvent = JSON.parse(data);
								this.onEvent(event);
							} catch (error) {
								console.error("Failed to parse entity event:", error);
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

				console.error("Entity events SSE connection error:", error);
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
						`Reconnecting entity events in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxAttempts})`,
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

export const eventsApi = {
	/**
	 * Subscribe to entity change events via SSE with automatic reconnection
	 */
	subscribeToEntityEvents: (
		onEvent: (event: EntityChangeEvent) => void,
		onError?: (error: Error) => void,
		onConnectionStateChange?: (
			state: "connecting" | "connected" | "disconnected" | "failed",
		) => void,
	): (() => void) => {
		const url = "/api/v1/events/stream";
		const manager = new EntityEventsReconnectionManager(
			url,
			onEvent,
			onError,
			onConnectionStateChange,
		);

		// Start connection asynchronously and store the cleanup function
		let unsubscribe: (() => void) | null = null;
		let isUnsubscribed = false;
		manager.connect().then((cleanup) => {
			unsubscribe = cleanup;
			// If unsubscribe was called before cleanup was ready, call it now
			if (isUnsubscribed) {
				cleanup();
			}
		});

		// Return cleanup function synchronously
		return () => {
			isUnsubscribed = true;
			if (unsubscribe) {
				unsubscribe();
			}
		};
	},
};
