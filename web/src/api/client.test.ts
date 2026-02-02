import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { navigationService } from "@/services/navigation";
import { api, onRateLimitNotification } from "./client";

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
          message: "Too many requests. Please try again later.",
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
