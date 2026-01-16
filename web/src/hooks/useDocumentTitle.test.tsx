import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import type { ReactNode } from "react";
import {
	afterAll,
	afterEach,
	beforeAll,
	beforeEach,
	describe,
	expect,
	it,
} from "vitest";
import { useDocumentTitle, useDynamicDocumentTitle } from "./useDocumentTitle";

// Setup MSW server
const server = setupServer(
	// Default handler that returns "Codex" as app name
	http.get("/api/v1/settings/branding", () => {
		return HttpResponse.json({ application_name: "Codex" });
	}),
);

beforeAll(() => server.listen({ onUnhandledRequest: "bypass" }));
afterEach(() => {
	server.resetHandlers();
	document.title = ""; // Reset document title between tests
});
afterAll(() => server.close());

// Helper to create wrapper with QueryClient
function createWrapper() {
	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				retry: false,
				gcTime: 0,
			},
		},
	});

	return function Wrapper({ children }: { children: ReactNode }) {
		return (
			<QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
		);
	};
}

describe("useDocumentTitle", () => {
	beforeEach(() => {
		document.title = "";
	});

	it("should set document title with page title and app name", () => {
		renderHook(() => useDocumentTitle("Libraries"), {
			wrapper: createWrapper(),
		});

		// Initially uses default app name (Codex) since branding hasn't loaded
		expect(document.title).toBe("Libraries - Codex");
	});

	it("should set document title to just app name when no page title provided", () => {
		renderHook(() => useDocumentTitle(), {
			wrapper: createWrapper(),
		});

		expect(document.title).toBe("Codex");
	});

	it("should set document title to just app name when page title is empty string", () => {
		renderHook(() => useDocumentTitle(""), {
			wrapper: createWrapper(),
		});

		// Empty string is falsy, so should just show app name
		expect(document.title).toBe("Codex");
	});

	it("should update document title when page title changes", () => {
		const { rerender } = renderHook(
			({ pageTitle }: { pageTitle?: string }) => useDocumentTitle(pageTitle),
			{
				wrapper: createWrapper(),
				initialProps: { pageTitle: "Home" },
			},
		);

		expect(document.title).toBe("Home - Codex");

		rerender({ pageTitle: "Settings" });

		expect(document.title).toBe("Settings - Codex");
	});

	it("should use custom app name from branding settings", async () => {
		server.use(
			http.get("/api/v1/settings/branding", () => {
				return HttpResponse.json({ application_name: "My Comics" });
			}),
		);

		const { rerender } = renderHook(() => useDocumentTitle("Libraries"), {
			wrapper: createWrapper(),
		});

		// Wait for branding to load and trigger re-render
		await new Promise((resolve) => setTimeout(resolve, 50));
		rerender();

		expect(document.title).toBe("Libraries - My Comics");
	});
});

describe("useDynamicDocumentTitle", () => {
	beforeEach(() => {
		document.title = "";
	});

	it("should use fallback title when page title is undefined", () => {
		renderHook(() => useDynamicDocumentTitle(undefined, "Loading..."), {
			wrapper: createWrapper(),
		});

		expect(document.title).toBe("Loading... - Codex");
	});

	it("should use page title when provided", () => {
		renderHook(() => useDynamicDocumentTitle("Book Title", "Loading..."), {
			wrapper: createWrapper(),
		});

		expect(document.title).toBe("Book Title - Codex");
	});

	it("should update when page title becomes available", () => {
		const { rerender } = renderHook(
			({ title }: { title: string | undefined }) =>
				useDynamicDocumentTitle(title, "Loading..."),
			{
				wrapper: createWrapper(),
				initialProps: { title: undefined as string | undefined },
			},
		);

		expect(document.title).toBe("Loading... - Codex");

		rerender({ title: "Actual Book Title" });

		expect(document.title).toBe("Actual Book Title - Codex");
	});

	it("should show just app name when both title and fallback are undefined", () => {
		renderHook(() => useDynamicDocumentTitle(undefined, undefined), {
			wrapper: createWrapper(),
		});

		expect(document.title).toBe("Codex");
	});
});
