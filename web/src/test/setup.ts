import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import "@testing-library/jest-dom/vitest";

// Mock console.debug to reduce test output noise
global.console.debug = vi.fn();

// Spy on console.error to filter out expected errors
const originalConsoleError = console.error;
global.console.error = vi.fn((...args: unknown[]) => {
	const message = args[0];

	// Suppress expected connection errors and React act() warnings during tests
	if (
		typeof message === 'string' && (
			message.includes('Task progress stream error') ||
			message.includes('ECONNREFUSED') ||
			message.includes('fetch failed') ||
			message.includes('not wrapped in act(')
		)
	) {
		return;
	}

	// For all other errors, call the original console.error
	originalConsoleError.apply(console, args);
});

// Cleanup after each test
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	localStorage.clear();
});

// Mock window.matchMedia
Object.defineProperty(window, "matchMedia", {
	writable: true,
	value: vi.fn().mockImplementation((query) => ({
		matches: false,
		media: query,
		onchange: null,
		addListener: vi.fn(),
		removeListener: vi.fn(),
		addEventListener: vi.fn(),
		removeEventListener: vi.fn(),
		dispatchEvent: vi.fn(),
	})),
});

// Mock IntersectionObserver
global.IntersectionObserver = class IntersectionObserver {
	constructor() {}
	disconnect() {}
	observe() {}
	takeRecords() {
		return [];
	}
	unobserve() {}
} as any;

// Mock ResizeObserver
global.ResizeObserver = class ResizeObserver {
	constructor() {}
	disconnect() {}
	observe() {}
	unobserve() {}
} as any;

// Mock EventSource for SSE tests
global.EventSource = class EventSource {
	url: string;
	withCredentials: boolean;
	readyState: number = 0;
	onmessage: ((event: MessageEvent) => void) | null = null;
	onerror: ((event: Event) => void) | null = null;
	onopen: ((event: Event) => void) | null = null;

	constructor(url: string, options?: { withCredentials?: boolean }) {
		this.url = url;
		this.withCredentials = options?.withCredentials ?? false;
	}

	close() {
		this.readyState = 2; // CLOSED
	}

	addEventListener() {}
	removeEventListener() {}
	dispatchEvent() {
		return false;
	}
} as any;
