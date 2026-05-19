import axios from "axios";
import { useAuthStore } from "@/store/authStore";
import type { components } from "@/types/api.generated";

type TokenPair = components["schemas"]["TokenPair"];

const REFRESH_URL = "/api/v1/auth/refresh";

let inFlight: Promise<string> | null = null;

/**
 * Exchange the stored refresh token for a fresh access+refresh pair.
 *
 * Concurrent callers share a single in-flight request so we never fire
 * parallel /auth/refresh calls (which would race the server-side rotation
 * and cause spurious family revocation). On success the auth store is
 * updated with the rotated pair and the new access token is returned.
 * On failure the cached promise is cleared so the next caller retries.
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
