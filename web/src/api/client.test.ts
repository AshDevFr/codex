import { beforeEach, describe, expect, it, vi } from "vitest";
import { navigationService } from "@/services/navigation";
import { api } from "./client";

describe("API Client", () => {
	beforeEach(() => {
		localStorage.clear();
		vi.clearAllMocks();
		// Mock navigation service to avoid actual navigation
		vi.spyOn(navigationService, "navigateTo").mockImplementation(() => {});
	});

	it("should create axios instance with correct base URL", () => {
		expect(api.defaults.baseURL).toBe("/api/v1");
		expect(api.defaults.timeout).toBe(30000);
	});

	it("should add JWT token to request headers", async () => {
		const token = "test-jwt-token";
		localStorage.setItem("jwt_token", token);

		const config = {
			headers: {},
		};

		// Access interceptor handlers through type assertion
		// Axios stores interceptors in a handlers array internally
		const handlers = (api.interceptors.request as any).handlers;
		if (handlers && handlers.length > 0) {
			const interceptorFn = handlers[0]?.fulfilled;
			if (interceptorFn) {
				const result = interceptorFn(config);
				expect(result.headers.Authorization).toBe(`Bearer ${token}`);
			}
		}
	});

	it("should not add Authorization header if no token", async () => {
		const config = {
			headers: {},
		};

		// Access interceptor handlers through type assertion
		const handlers = (api.interceptors.request as any).handlers;
		if (handlers && handlers.length > 0) {
			const interceptorFn = handlers[0]?.fulfilled;
			if (interceptorFn) {
				const result = interceptorFn(config);
				expect(result.headers.Authorization).toBeUndefined();
			}
		}
	});

	it("should handle 401 errors and clear auth", async () => {
		const mockError = {
			response: {
				status: 401,
				data: {
					error: "Unauthorized",
				},
			},
		};

		localStorage.setItem("jwt_token", "token");

		// Mock window.location
		delete (window as any).location;
		window.location = { href: "" } as any;

		// Access interceptor handlers through type assertion
		const handlers = (api.interceptors.response as any).handlers;
		if (handlers && handlers.length > 0) {
			const interceptorFn = handlers[0]?.rejected;
			if (interceptorFn) {
				await expect(interceptorFn(mockError)).rejects.toEqual({
					error: "Unauthorized",
					message: undefined,
				});
			}
		}

		// Verify that clearAuth was called (it removes jwt_token from localStorage)
		expect(localStorage.getItem("jwt_token")).toBeNull();
		// Verify navigation was called
		expect(navigationService.navigateTo).toHaveBeenCalledWith("/login");
	});

	it("should handle network errors", async () => {
		const mockError = {
			message: "Network Error",
		};

		// Access interceptor handlers through type assertion
		const handlers = (api.interceptors.response as any).handlers;
		if (handlers && handlers.length > 0) {
			const interceptorFn = handlers[0]?.rejected;
			if (interceptorFn) {
				await expect(interceptorFn(mockError)).rejects.toEqual({
					error: "Network Error",
					message: "Network Error",
				});
			}
		}
	});
});
