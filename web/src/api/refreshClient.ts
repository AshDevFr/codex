import axios, { type AxiosError } from "axios";
import { useAuthStore } from "@/store/authStore";
import type { components } from "@/types/api.generated";

type TokenPair = components["schemas"]["TokenPair"];

const REFRESH_URL = "/api/v1/auth/refresh";

/**
 * Raised when /auth/refresh does not return a fresh token pair.
 *
 * `transient` distinguishes "server hiccup, try again later" from "this
 * refresh token is dead, the user must reauthenticate". The 401-interceptor
 * uses it to decide whether to clear the session: a network blip or 5xx
 * must never log the user out, otherwise an unstable connection turns into
 * an unprovoked sign-out mid-read.
 */
export class RefreshFailedError extends Error {
  readonly transient: boolean;
  readonly status?: number;
  readonly cause?: unknown;
  constructor(opts: {
    message: string;
    transient: boolean;
    status?: number;
    cause?: unknown;
  }) {
    super(opts.message);
    this.name = "RefreshFailedError";
    this.transient = opts.transient;
    this.status = opts.status;
    this.cause = opts.cause;
  }
}

/**
 * No-response (network down, CORS abort, DNS), 5xx, or 429 are treated as
 * transient. Anything else (notably 4xx auth failures from the server) is
 * definitive and must drop the session.
 */
function classifyRefreshError(err: unknown): RefreshFailedError {
  const axErr = err as AxiosError | undefined;
  const status = axErr?.response?.status;
  const hasResponse = axErr?.response !== undefined;
  const transient =
    !hasResponse ||
    (typeof status === "number" && (status >= 500 || status === 429));
  return new RefreshFailedError({
    message: `auth/refresh failed${status ? ` (HTTP ${status})` : ""}`,
    transient,
    status,
    cause: err,
  });
}

let inFlight: Promise<string> | null = null;

/**
 * Exchange the stored refresh token for a fresh access+refresh pair.
 *
 * Concurrent callers share a single in-flight request so we never fire
 * parallel /auth/refresh calls (which would race the server-side rotation
 * and cause spurious family revocation). On success the auth store is
 * updated with the rotated pair and the new access token is returned.
 * On failure the cached promise is cleared so the next caller retries.
 *
 * Failures are wrapped in {@link RefreshFailedError} so the interceptor can
 * tell a definitive auth failure (kick the user out) apart from a transient
 * network/server hiccup (leave the session alone, surface the original
 * error).
 */
export function getFreshAccessToken(): Promise<string> {
  if (inFlight) {
    return inFlight;
  }

  const refreshToken =
    useAuthStore.getState().refreshToken ??
    localStorage.getItem("jwt_refresh_token");

  if (!refreshToken) {
    return Promise.reject(new Error("No refresh token available"));
  }

  inFlight = axios
    .post<TokenPair>(
      REFRESH_URL,
      { refreshToken },
      {
        // Send cookies so the rotated `auth_token` cookie reaches us.
        withCredentials: true,
        headers: { "Content-Type": "application/json" },
      },
    )
    .then((response) => {
      const pair = response.data;
      useAuthStore.getState().updateTokens(pair.accessToken, pair.refreshToken);
      return pair.accessToken;
    })
    .catch((err) => {
      throw classifyRefreshError(err);
    })
    .finally(() => {
      inFlight = null;
    });

  return inFlight;
}

/**
 * Test-only: reset the in-flight refresh slot.
 *
 * Exposed for unit tests that need to ensure a clean slate between cases.
 * Production code should never call this.
 */
export function __resetInFlightRefresh() {
  inFlight = null;
}
