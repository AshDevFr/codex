import axios from "axios";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAuthStore } from "@/store/authStore";
import { __resetInFlightRefresh, getFreshAccessToken } from "./refreshClient";

vi.mock("axios", async () => {
  const actual = await vi.importActual<typeof import("axios")>("axios");
  return {
    ...actual,
    default: {
      ...actual.default,
      post: vi.fn(),
    },
  };
});

const mockedAxiosPost = vi.mocked(axios.post);

describe("refreshClient.getFreshAccessToken", () => {
  beforeEach(() => {
    __resetInFlightRefresh();
    mockedAxiosPost.mockReset();
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
    localStorage.clear();
    localStorage.setItem("jwt_refresh_token", "old-refresh");
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

  it("calls /auth/refresh and updates the store with the new pair", async () => {
    mockedAxiosPost.mockResolvedValueOnce({
      data: {
        accessToken: "new-access",
        refreshToken: "new-refresh",
        tokenType: "Bearer",
        expiresIn: 86400,
      },
    });

    const token = await getFreshAccessToken();

    expect(token).toBe("new-access");
    expect(mockedAxiosPost).toHaveBeenCalledTimes(1);
    expect(mockedAxiosPost).toHaveBeenCalledWith(
      "/api/v1/auth/refresh",
      { refreshToken: "old-refresh" },
      expect.objectContaining({ withCredentials: true }),
    );

    const state = useAuthStore.getState();
    expect(state.token).toBe("new-access");
    expect(state.refreshToken).toBe("new-refresh");
    expect(localStorage.getItem("jwt_token")).toBe("new-access");
    expect(localStorage.getItem("jwt_refresh_token")).toBe("new-refresh");
  });

  it("shares a single in-flight refresh across concurrent callers", async () => {
    let resolveResponse: (value: unknown) => void = () => {};
    const pending = new Promise<unknown>((resolve) => {
      resolveResponse = resolve;
    });
    mockedAxiosPost.mockReturnValueOnce(pending as any);

    const first = getFreshAccessToken();
    const second = getFreshAccessToken();
    const third = getFreshAccessToken();

    expect(mockedAxiosPost).toHaveBeenCalledTimes(1);

    resolveResponse({
      data: {
        accessToken: "shared-access",
        refreshToken: "shared-refresh",
        tokenType: "Bearer",
        expiresIn: 86400,
      },
    });

    const [a, b, c] = await Promise.all([first, second, third]);
    expect(a).toBe("shared-access");
    expect(b).toBe("shared-access");
    expect(c).toBe("shared-access");
    expect(mockedAxiosPost).toHaveBeenCalledTimes(1);
  });

  it("rejects (and clears the in-flight slot) when the refresh fails", async () => {
    mockedAxiosPost.mockRejectedValueOnce(new Error("network down"));

    await expect(getFreshAccessToken()).rejects.toThrow();

    // After failure, a subsequent call should launch a new refresh, not reuse the
    // rejected promise.
    mockedAxiosPost.mockResolvedValueOnce({
      data: {
        accessToken: "recovered",
        refreshToken: "recovered-refresh",
        tokenType: "Bearer",
        expiresIn: 86400,
      },
    });
    const token = await getFreshAccessToken();
    expect(token).toBe("recovered");
    expect(mockedAxiosPost).toHaveBeenCalledTimes(2);
  });

  it("rejects immediately when no refresh token is available", async () => {
    useAuthStore.setState({ refreshToken: null });
    localStorage.removeItem("jwt_refresh_token");

    await expect(getFreshAccessToken()).rejects.toThrow();
    expect(mockedAxiosPost).not.toHaveBeenCalled();
  });
});
