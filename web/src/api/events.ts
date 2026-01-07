import type { EntityChangeEvent } from "@/types/events";

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "http://localhost:3000";

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
	private onConnectionStateChange?: (state: 'connecting' | 'connected' | 'disconnected' | 'failed') => void;

	constructor(
		url: string,
		onEvent: (event: EntityChangeEvent) => void,
		onError?: (error: Error) => void,
		onConnectionStateChange?: (state: 'connecting' | 'connected' | 'disconnected' | 'failed') => void,
	) {
		this.url = url;
		this.onEvent = onEvent;
		this.onError = onError;
		this.onConnectionStateChange = onConnectionStateChange;
	}

	async connect(): Promise<() => void> {
		this.onConnectionStateChange?.('connecting');

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
					throw new Error(`SSE connection failed: ${response.status} ${response.statusText}`);
				}

				if (!response.body) {
					throw new Error("Response body is null");
				}

				// Reset reconnection counter on successful connection
				this.reconnectAttempts = 0;
				this.onConnectionStateChange?.('connected');

				const reader = response.body.getReader();
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
				reader.cancel();
			} catch (error) {
				if (!this.active) return;

				console.error("Entity events SSE connection error:", error);
				this.onConnectionStateChange?.('disconnected');

				// Attempt reconnection with exponential backoff
				if (this.reconnectAttempts < this.maxAttempts) {
					const delay = Math.min(
						this.baseDelay * (2 ** this.reconnectAttempts),
						this.maxDelay
					);
					this.reconnectAttempts++;

					console.log(`Reconnecting entity events in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxAttempts})`);

					this.reconnectTimer = setTimeout(() => {
						attemptConnection();
					}, delay);
				} else {
					this.onConnectionStateChange?.('failed');
					this.onError?.(new Error("Max reconnection attempts reached"));
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
		};
	}
}

export const eventsApi = {
	/**
	 * Subscribe to entity change events via SSE with automatic reconnection
	 */
	subscribeToEntityEvents: async (
		onEvent: (event: EntityChangeEvent) => void,
		onError?: (error: Error) => void,
		onConnectionStateChange?: (state: 'connecting' | 'connected' | 'disconnected' | 'failed') => void,
	): Promise<() => void> => {
		const url = `${API_BASE_URL}/api/v1/events/stream`;
		const manager = new EntityEventsReconnectionManager(
			url,
			onEvent,
			onError,
			onConnectionStateChange
		);
		return manager.connect();
	},
};
