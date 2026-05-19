import axios, {
  type AxiosError,
  type AxiosInstance,
  type InternalAxiosRequestConfig,
} from "axios";
import { navigationService } from "@/services/navigation";
import { useAuthStore } from "@/store/authStore";
import type { ApiError } from "@/types";
import { getFreshAccessToken } from "./refreshClient";

// Rate limit retry configuration
const RATE_LIMIT_MAX_RETRIES = 3;
const RATE_LIMIT_DEFAULT_RETRY_AFTER = 5; // seconds
const RATE_LIMIT_NOTIFICATION_THRESHOLD = 5; // Show notification if retry-after > 5 seconds

// Extend axios config to track retry state for rate limit and refresh-token flows.
interface RetriableRequestConfig extends InternalAxiosRequestConfig {
  _rateLimitRetryCount?: number;
  _refreshRetried?: boolean;
}

// Endpoints that must not trigger a refresh-on-401 (the refresh endpoint itself
// would be an infinite loop, and login/logout paths legitimately 401 on bad
// credentials and should surface that to the UI).
const AUTH_PATHS_NO_REFRESH = ["/auth/refresh", "/auth/login", "/auth/logout"];

function isAuthPath(url: string | undefined): boolean {
  if (!url) return false;
  return AUTH_PATHS_NO_REFRESH.some((path) => url.includes(path));
}

// Event for rate limit notifications (can be subscribed to by UI components)
type RateLimitEventHandler = (retryAfterSeconds: number) => void;
let rateLimitNotificationHandler: RateLimitEventHandler | null = null;

/**
 * Register a handler to be called when rate limiting requires extended waiting.
 * This allows the UI to show a notification to the user.
 */
export function onRateLimitNotification(handler: RateLimitEventHandler | null) {
  rateLimitNotificationHandler = handler;
}

/**
 * Extract retry-after value from 429 response.
 * Checks Retry-After header and response body.
 */
function getRetryAfterSeconds(error: AxiosError<ApiError>): number {
  // Try Retry-After header first (standard HTTP header)
  const retryAfterHeader = error.response?.headers?.["retry-after"];
  if (retryAfterHeader) {
    const parsed = Number.parseInt(retryAfterHeader, 10);
    if (!Number.isNaN(parsed) && parsed > 0) {
      return parsed;
    }
  }

  // Fall back to response body retry_after field
  const bodyRetryAfter = (error.response?.data as { retry_after?: number })
    ?.retry_after;
  if (typeof bodyRetryAfter === "number" && bodyRetryAfter > 0) {
    return bodyRetryAfter;
  }

  return RATE_LIMIT_DEFAULT_RETRY_AFTER;
}

/**
 * Sleep for specified milliseconds
 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// Create axios instance with base configuration
export const api: AxiosInstance = axios.create({
  baseURL: "/api/v1",
  timeout: 30000,
  headers: {
    "Content-Type": "application/json",
  },
  // IMPORTANT: Send cookies with requests (required for cookie-based image auth)
  withCredentials: true,
});

// Request interceptor to add auth token
api.interceptors.request.use(
  (config) => {
    const token = localStorage.getItem("jwt_token");
    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => {
    return Promise.reject(error);
  },
);

// Response interceptor to handle errors (including 429 rate limit retry and
// 401 transparent refresh-token rotation).
api.interceptors.response.use(
  (response) => response,
  async (error: AxiosError<ApiError>) => {
    const config = error.config as RetriableRequestConfig | undefined;

    // Handle 429 Too Many Requests with automatic retry
    if (error.response?.status === 429 && config) {
      const retryCount = config._rateLimitRetryCount ?? 0;

      if (retryCount < RATE_LIMIT_MAX_RETRIES) {
        const retryAfterSeconds = getRetryAfterSeconds(error);

        // Notify UI if wait time is significant
        if (
          retryAfterSeconds >= RATE_LIMIT_NOTIFICATION_THRESHOLD &&
          rateLimitNotificationHandler
        ) {
          rateLimitNotificationHandler(retryAfterSeconds);
        }

        // Wait for the retry-after period
        await sleep(retryAfterSeconds * 1000);

        // Increment retry count and retry the request
        config._rateLimitRetryCount = retryCount + 1;
        return api.request(config);
      }

      // Max retries exceeded - return rate limit error, preserving the server's message
      return Promise.reject({
        error: "rate_limit_exceeded",
        message:
          error.response?.data?.message ||
          "Too many requests. Please try again later.",
      } as ApiError);
    }

    // Handle 401 Unauthorized.
    // Try a single refresh-token exchange first; on failure (or for auth
    // endpoints themselves) fall through to the legacy clear-auth + redirect.
    if (
      error.response?.status === 401 &&
      config &&
      !config._refreshRetried &&
      !isAuthPath(config.url)
    ) {
      const { refreshToken } = useAuthStore.getState();
      if (refreshToken) {
        try {
          const newAccessToken = await getFreshAccessToken();
          config._refreshRetried = true;
          config.headers = config.headers ?? {};
          (config.headers as Record<string, string>).Authorization =
            `Bearer ${newAccessToken}`;
          return api.request(config);
        } catch {
          // Refresh failed; fall through to the clear-auth path below.
        }
      }
    }

    if (error.response) {
      const apiError: ApiError = {
        error: error.response.data?.error || "An error occurred",
        message: error.response.data?.message || error.message,
      };

      // Handle 401 Unauthorized - clear auth state and redirect to login.
      // We reach this either because no refresh token was available, the
      // refresh itself failed, or the retried request still 401'd.
      if (error.response.status === 401) {
        const { clearAuth } = useAuthStore.getState();
        clearAuth();
        navigationService.navigateTo("/login");
      }

      return Promise.reject(apiError);
    }

    return Promise.reject({
      error: "Network Error",
      message: error.message,
    } as ApiError);
  },
);
