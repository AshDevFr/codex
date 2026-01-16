import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import type { ReactNode } from "react";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { DEFAULT_APP_NAME, useAppName, useBranding } from "./useAppName";

// Setup MSW server
const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "bypass" }));
afterEach(() => server.resetHandlers());
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

describe("useAppName", () => {
	it("should return default app name while loading", () => {
		server.use(
			http.get("/api/v1/settings/branding", async () => {
				// Delay response to ensure we test the loading state
				await new Promise((resolve) => setTimeout(resolve, 100));
				return HttpResponse.json({ application_name: "Custom App" });
			}),
		);

		const { result } = renderHook(() => useAppName(), {
			wrapper: createWrapper(),
		});

		// Should return default value immediately
		expect(result.current).toBe(DEFAULT_APP_NAME);
	});

	it("should return custom app name after loading", async () => {
		server.use(
			http.get("/api/v1/settings/branding", () => {
				return HttpResponse.json({ application_name: "My Library" });
			}),
		);

		const { result } = renderHook(() => useAppName(), {
			wrapper: createWrapper(),
		});

		await waitFor(() => {
			expect(result.current).toBe("My Library");
		});
	});

	it("should return default app name on error", async () => {
		server.use(
			http.get("/api/v1/settings/branding", () => {
				return new HttpResponse(null, { status: 500 });
			}),
		);

		const { result } = renderHook(() => useAppName(), {
			wrapper: createWrapper(),
		});

		// Should return default value after error
		await waitFor(() => {
			expect(result.current).toBe(DEFAULT_APP_NAME);
		});
	});

	it("should return default app name when response has no application_name", async () => {
		server.use(
			http.get("/api/v1/settings/branding", () => {
				return HttpResponse.json({});
			}),
		);

		const { result } = renderHook(() => useAppName(), {
			wrapper: createWrapper(),
		});

		await waitFor(() => {
			expect(result.current).toBe(DEFAULT_APP_NAME);
		});
	});
});

describe("useBranding", () => {
	it("should return loading state initially", () => {
		server.use(
			http.get("/api/v1/settings/branding", async () => {
				await new Promise((resolve) => setTimeout(resolve, 100));
				return HttpResponse.json({ application_name: "Custom App" });
			}),
		);

		const { result } = renderHook(() => useBranding(), {
			wrapper: createWrapper(),
		});

		expect(result.current.isLoading).toBe(true);
		expect(result.current.appName).toBe(DEFAULT_APP_NAME);
		expect(result.current.error).toBe(null);
		expect(result.current.isError).toBe(false);
	});

	it("should return app name and success state after loading", async () => {
		server.use(
			http.get("/api/v1/settings/branding", () => {
				return HttpResponse.json({ application_name: "My Comics" });
			}),
		);

		const { result } = renderHook(() => useBranding(), {
			wrapper: createWrapper(),
		});

		await waitFor(() => {
			expect(result.current.isLoading).toBe(false);
		});

		expect(result.current.appName).toBe("My Comics");
		expect(result.current.error).toBe(null);
		expect(result.current.isError).toBe(false);
	});

	it("should return error state on failure", async () => {
		server.use(
			http.get("/api/v1/settings/branding", () => {
				return new HttpResponse(null, { status: 500 });
			}),
		);

		const { result } = renderHook(() => useBranding(), {
			wrapper: createWrapper(),
		});

		// Wait for the query to finish (loading becomes false)
		// Note: The hook has retry: 1, so it tries twice before erroring
		await waitFor(
			() => {
				expect(result.current.isLoading).toBe(false);
			},
			{ timeout: 3000 },
		);

		// After all retries, should have error state
		expect(result.current.isError).toBe(true);
		expect(result.current.appName).toBe(DEFAULT_APP_NAME);
	});
});
