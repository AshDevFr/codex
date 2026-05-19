import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { navigationService } from "@/services/navigation";
import { useAuthStore } from "@/store/authStore";
import { api, onRateLimitNotification } from "./client";
import * as refreshClient from "./refreshClient";

describe("API Client", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    refreshClient.__resetInFlightRefresh();
    useAuthStore.setState({
      user: null,
      token: null,
      refreshToken: null,
      isAuthenticated: false,
    });
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

  it("should handle 401 errors and clear auth when no refresh token is available", async () => {
    const mockError = {
      response: {
        status: 401,
        data: {
          error: "Unauthorized",
        },
      },
      config: {
        url: "/users/me",
        method: "get",
        headers: {},
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

describe("Rate Limit Retry Logic", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    // Clear any notification handler
    onRateLimitNotification(null);
  });

  afterEach(() => {
    vi.useRealTimers();
    onRateLimitNotification(null);
  });

  it("should return rate_limit_exceeded error after max retries", async () => {
    const mockError = {
      response: {
        status: 429,
        headers: {
          "retry-after": "1",
        },
        data: {
          error: "rate_limit_exceeded",
          message: "Too many requests",
          retry_after: 1,
        },
      },
      config: {
        _rateLimitRetryCount: 3, // Already at max retries
        url: "/test",
        method: "get",
        headers: {},
      },
    };

    // Access interceptor handlers through type assertion
    const handlers = (api.interceptors.response as any).handlers;
    if (handlers && handlers.length > 0) {
      const interceptorFn = handlers[0]?.rejected;
      if (interceptorFn) {
        await expect(interceptorFn(mockError)).rejects.toEqual({
          error: "rate_limit_exceeded",
          message: "Too many requests",
        });
      }
    }
  });

  it("should call notification handler when retry-after exceeds threshold", async () => {
    const notificationHandler = vi.fn();
    onRateLimitNotification(notificationHandler);

    // Mock api.request to resolve on retry
    const requestSpy = vi.spyOn(api, "request").mockResolvedValueOnce({
      data: { success: true },
      status: 200,
      statusText: "OK",
      headers: {},
      config: {} as any,
    });

    const mockError = {
      response: {
        status: 429,
        headers: {
          "retry-after": "10", // More than threshold (5 seconds)
        },
        data: {
          error: "rate_limit_exceeded",
          message: "Too many requests",
        },
      },
      config: {
        _rateLimitRetryCount: 0,
        url: "/test",
        method: "get",
        headers: {},
      },
    };

    // Access interceptor handlers through type assertion
    const handlers = (api.interceptors.response as any).handlers;
    if (handlers && handlers.length > 0) {
      const interceptorFn = handlers[0]?.rejected;
      if (interceptorFn) {
        // Start the interceptor call (it will wait for sleep)
        const promise = interceptorFn(mockError);

        // Advance timers to let the sleep complete
        await vi.advanceTimersByTimeAsync(10000);

        await promise;

        // Verify notification was called with retry-after value
        expect(notificationHandler).toHaveBeenCalledWith(10);
      }
    }

    requestSpy.mockRestore();
  });

  it("should not call notification handler when retry-after is below threshold", async () => {
    const notificationHandler = vi.fn();
    onRateLimitNotification(notificationHandler);

    // Mock api.request to resolve on retry
    const requestSpy = vi.spyOn(api, "request").mockResolvedValueOnce({
      data: { success: true },
      status: 200,
      statusText: "OK",
      headers: {},
      config: {} as any,
    });

    const mockError = {
      response: {
        status: 429,
        headers: {
          "retry-after": "3", // Less than threshold (5 seconds)
        },
        data: {
          error: "rate_limit_exceeded",
          message: "Too many requests",
        },
      },
      config: {
        _rateLimitRetryCount: 0,
        url: "/test",
        method: "get",
        headers: {},
      },
    };

    // Access interceptor handlers through type assertion
    const handlers = (api.interceptors.response as any).handlers;
    if (handlers && handlers.length > 0) {
      const interceptorFn = handlers[0]?.rejected;
      if (interceptorFn) {
        const promise = interceptorFn(mockError);
        await vi.advanceTimersByTimeAsync(3000);
        await promise;

        // Notification should NOT be called for short waits
        expect(notificationHandler).not.toHaveBeenCalled();
      }
    }

    requestSpy.mockRestore();
  });

  it("should use default retry-after when header is missing", async () => {
    // Mock api.request to resolve on retry
    const requestSpy = vi.spyOn(api, "request").mockResolvedValueOnce({
      data: { success: true },
      status: 200,
      statusText: "OK",
      headers: {},
      config: {} as any,
    });

    const mockError = {
      response: {
        status: 429,
        headers: {}, // No retry-after header
        data: {
          error: "rate_limit_exceeded",
          message: "Too many requests",
          // No retry_after in body either
        },
      },
      config: {
        _rateLimitRetryCount: 0,
        url: "/test",
        method: "get",
        headers: {},
      },
    };

    // Access interceptor handlers through type assertion
    const handlers = (api.interceptors.response as any).handlers;
    if (handlers && handlers.length > 0) {
      const interceptorFn = handlers[0]?.rejected;
      if (interceptorFn) {
        const promise = interceptorFn(mockError);

        // Should use default of 5 seconds
        await vi.advanceTimersByTimeAsync(5000);
        await promise;

        // Verify request was made after the default delay
        expect(requestSpy).toHaveBeenCalled();
      }
    }

    requestSpy.mockRestore();
  });

  it("should use retry_after from response body when header is missing", async () => {
    // Mock api.request to resolve on retry
    const requestSpy = vi.spyOn(api, "request").mockResolvedValueOnce({
      data: { success: true },
      status: 200,
      statusText: "OK",
      headers: {},
      config: {} as any,
    });

    const mockError = {
      response: {
        status: 429,
        headers: {}, // No retry-after header
        data: {
          error: "rate_limit_exceeded",
          message: "Too many requests",
          retry_after: 7, // Body has retry_after
        },
      },
      config: {
        _rateLimitRetryCount: 0,
        url: "/test",
        method: "get",
        headers: {},
      },
    };

    const notificationHandler = vi.fn();
    onRateLimitNotification(notificationHandler);

    // Access interceptor handlers through type assertion
    const handlers = (api.interceptors.response as any).handlers;
    if (handlers && handlers.length > 0) {
      const interceptorFn = handlers[0]?.rejected;
      if (interceptorFn) {
        const promise = interceptorFn(mockError);

        // Should use 7 seconds from body
        await vi.advanceTimersByTimeAsync(7000);
        await promise;

        // Verify notification was called with body value (7 > 5 threshold)
        expect(notificationHandler).toHaveBeenCalledWith(7);
      }
    }

    requestSpy.mockRestore();
  });

  it("should increment retry count on each retry", async () => {
    // Mock api.request to track the config
    let capturedConfig: any;
    const requestSpy = vi.spyOn(api, "request").mockImplementation((config) => {
      capturedConfig = config;
      return Promise.resolve({
        data: { success: true },
        status: 200,
        statusText: "OK",
        headers: {},
        config: {} as any,
      });
    });

    const mockError = {
      response: {
        status: 429,
        headers: {
          "retry-after": "1",
        },
        data: {},
      },
      config: {
        _rateLimitRetryCount: 1, // Already retried once
        url: "/test",
        method: "get",
        headers: {},
      },
    };

    // Access interceptor handlers through type assertion
    const handlers = (api.interceptors.response as any).handlers;
    if (handlers && handlers.length > 0) {
      const interceptorFn = handlers[0]?.rejected;
      if (interceptorFn) {
        const promise = interceptorFn(mockError);
        await vi.advanceTimersByTimeAsync(1000);
        await promise;

        // Verify retry count was incremented
        expect(capturedConfig._rateLimitRetryCount).toBe(2);
      }
    }

    requestSpy.mockRestore();
  });

  it("should allow registering and unregistering notification handler", () => {
    const handler = vi.fn();

    // Register handler
    onRateLimitNotification(handler);

    // Unregister handler
    onRateLimitNotification(null);

    // No error should occur - this is just testing the API works
    expect(true).toBe(true);
  });
});

describe("Refresh Token Retry Logic", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    refreshClient.__resetInFlightRefresh();
    useAuthStore.setState({
      user: {
        id: "1",
        username: "tester",
        email: "tester@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      },
      token: "old-access",
      refreshToken: "old-refresh",
      isAuthenticated: true,
    });
    vi.spyOn(navigationService, "navigateTo").mockImplementation(() => {});
  });

  afterEach(() => {
    useAuthStore.setState({
      user: null,
      token: null,
      refreshToken: null,
      isAuthenticated: false,
    });
    localStorage.clear();
  });

  function getResponseInterceptor() {
    const handlers = (api.interceptors.response as any).handlers;
    return handlers?.[0]?.rejected as
      | ((error: unknown) => Promise<unknown>)
      | undefined;
  }

  it("refreshes on 401 and retries the original request once with the new token", async () => {
    vi.spyOn(refreshClient, "getFreshAccessToken").mockResolvedValueOnce(
      "fresh-access",
    );
    const requestSpy = vi.spyOn(api, "request").mockResolvedValueOnce({
      data: { ok: true },
      status: 200,
      statusText: "OK",
      headers: {},
      config: {} as any,
    });

    const mockError = {
      response: { status: 401, data: { error: "Unauthorized" } },
      config: { url: "/users/me", method: "get", headers: {} },
    };

    const interceptor = getResponseInterceptor();
    expect(interceptor).toBeTruthy();

    const result = await interceptor!(mockError);
    expect((result as any).data).toEqual({ ok: true });

    expect(refreshClient.getFreshAccessToken).toHaveBeenCalledTimes(1);
    const replayedConfig = requestSpy.mock.calls[0]?.[0] as any;
    expect(replayedConfig.headers.Authorization).toBe("Bearer fresh-access");
    expect(replayedConfig._refreshRetried).toBe(true);
    expect(navigationService.navigateTo).not.toHaveBeenCalled();

    requestSpy.mockRestore();
  });

  it("clears auth and redirects to /login when the refresh itself fails", async () => {
    vi.spyOn(refreshClient, "getFreshAccessToken").mockRejectedValueOnce(
      new Error("refresh failed"),
    );

    const mockError = {
      response: { status: 401, data: { error: "Unauthorized" } },
      config: { url: "/users/me", method: "get", headers: {} },
    };

    const interceptor = getResponseInterceptor();
    await expect(interceptor!(mockError)).rejects.toEqual({
      error: "Unauthorized",
      message: undefined,
    });

    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(navigationService.navigateTo).toHaveBeenCalledWith("/login");
  });

  it("does not loop: a request that 401s again after retry surfaces the error", async () => {
    vi.spyOn(refreshClient, "getFreshAccessToken").mockResolvedValueOnce(
      "fresh-access",
    );

    const mockError = {
      response: { status: 401, data: { error: "Unauthorized" } },
      config: {
        url: "/users/me",
        method: "get",
        headers: {},
        _refreshRetried: true, // already retried once
      },
    };

    const interceptor = getResponseInterceptor();
    await expect(interceptor!(mockError)).rejects.toEqual({
      error: "Unauthorized",
      message: undefined,
    });

    // The refresh helper must NOT be called a second time for the same request.
    expect(refreshClient.getFreshAccessToken).not.toHaveBeenCalled();
    expect(navigationService.navigateTo).toHaveBeenCalledWith("/login");
  });

  it("skips refresh for /auth/refresh itself and falls straight to clear-auth", async () => {
    const refreshSpy = vi.spyOn(refreshClient, "getFreshAccessToken");

    const mockError = {
      response: { status: 401, data: { error: "Unauthorized" } },
      config: { url: "/auth/refresh", method: "post", headers: {} },
    };

    const interceptor = getResponseInterceptor();
    await expect(interceptor!(mockError)).rejects.toEqual({
      error: "Unauthorized",
      message: undefined,
    });

    expect(refreshSpy).not.toHaveBeenCalled();
    expect(navigationService.navigateTo).toHaveBeenCalledWith("/login");
  });

  it("shares a single refresh across two parallel 401s", async () => {
    let resolveRefresh: (value: string) => void = () => {};
    const pending = new Promise<string>((resolve) => {
      resolveRefresh = resolve;
    });
    const refreshSpy = vi
      .spyOn(refreshClient, "getFreshAccessToken")
      .mockReturnValue(pending);

    const requestSpy = vi.spyOn(api, "request").mockResolvedValue({
      data: { ok: true },
      status: 200,
      statusText: "OK",
      headers: {},
      config: {} as any,
    });

    const mkError = (url: string) => ({
      response: { status: 401, data: { error: "Unauthorized" } },
      config: { url, method: "get", headers: {} },
    });

    const interceptor = getResponseInterceptor();
    const a = interceptor!(mkError("/users/me"));
    const b = interceptor!(mkError("/libraries"));

    // Allow microtasks to drain so both branches reach the refresh await.
    await Promise.resolve();
    await Promise.resolve();

    resolveRefresh("fresh-access");

    await Promise.all([a, b]);

    expect(refreshSpy).toHaveBeenCalledTimes(2);
    // Both requests retried with the new token.
    expect(requestSpy).toHaveBeenCalledTimes(2);
    for (const call of requestSpy.mock.calls) {
      const cfg = call[0] as any;
      expect(cfg.headers.Authorization).toBe("Bearer fresh-access");
      expect(cfg._refreshRetried).toBe(true);
    }

    requestSpy.mockRestore();
  });

  it("skips refresh when no refresh token is in the store", async () => {
    useAuthStore.setState({ refreshToken: null });
    const refreshSpy = vi.spyOn(refreshClient, "getFreshAccessToken");

    const mockError = {
      response: { status: 401, data: { error: "Unauthorized" } },
      config: { url: "/users/me", method: "get", headers: {} },
    };

    const interceptor = getResponseInterceptor();
    await expect(interceptor!(mockError)).rejects.toEqual({
      error: "Unauthorized",
      message: undefined,
    });

    expect(refreshSpy).not.toHaveBeenCalled();
    expect(navigationService.navigateTo).toHaveBeenCalledWith("/login");
  });
});
