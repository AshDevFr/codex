import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import "@testing-library/jest-dom/vitest";

// Mock console.debug and console.log to reduce test output noise
// Replace them with no-op functions at module load time
// Use both console and global.console to ensure mocks apply everywhere
const noop = () => {};
global.console.debug = noop;
global.console.log = noop;
console.debug = noop;
console.log = noop;

// Spy on console.error to filter out expected errors
const originalConsoleError = console.error;
global.console.error = vi.fn((...args: unknown[]) => {
	const message = args[0];

	// Suppress expected connection errors and React act() warnings during tests
	if (typeof message === "string") {
		if (
			message.includes("Task progress stream error") ||
			message.includes("ECONNREFUSED") ||
			message.includes("fetch failed") ||
			message.includes("not wrapped in act(") ||
			message.includes("AggregateError")
		) {
			return;
		}
	}

	// Suppress AggregateError objects (jsdom XMLHttpRequest errors)
	if (
		message instanceof Error &&
		(message.name === "AggregateError" || message.constructor.name === "AggregateError")
	) {
		return;
	}

	// For all other errors, call the original console.error
	originalConsoleError.apply(console, args);
});

// Suppress unhandled AggregateError from jsdom XMLHttpRequest (expected in test environment)
// These occur when components try to make HTTP requests that aren't properly mocked
process.on("unhandledRejection", (reason) => {
	// Suppress AggregateError from jsdom's XMLHttpRequest implementation
	if (
		reason instanceof AggregateError ||
		(reason &&
			typeof reason === "object" &&
			"constructor" in reason &&
			reason.constructor.name === "AggregateError")
	) {
		// Silently suppress - this is expected when HTTP requests aren't mocked
		return;
	}
	// For other unhandled rejections, let them through (vitest will handle them)
});

// Cleanup after each test
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	// Restore console mocks after clearAllMocks (which might clear them)
	global.console.debug = noop;
	global.console.log = noop;
	console.debug = noop;
	console.log = noop;
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
