import { beforeEach, describe, expect, it } from "vitest";
import type { User } from "@/types";
import { useAuthStore } from "./authStore";

describe("authStore", () => {
  beforeEach(() => {
    // Reset store state before each test
    useAuthStore.setState({
      user: null,
      token: null,
      refreshToken: null,
      isAuthenticated: false,
    });
    localStorage.clear();
  });

  it("should have initial state", () => {
    const state = useAuthStore.getState();
    expect(state.user).toBeNull();
    expect(state.token).toBeNull();
    expect(state.refreshToken).toBeNull();
    expect(state.isAuthenticated).toBe(false);
  });

  it("should set auth state with user, token, and refresh token", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };
    const mockToken = "test-jwt-token";
    const mockRefreshToken = "test-refresh-token";

    useAuthStore.getState().setAuth(mockUser, mockToken, mockRefreshToken);

    const state = useAuthStore.getState();
    expect(state.user).toEqual(mockUser);
    expect(state.token).toBe(mockToken);
    expect(state.refreshToken).toBe(mockRefreshToken);
    expect(state.isAuthenticated).toBe(true);
    expect(localStorage.getItem("jwt_token")).toBe(mockToken);
    expect(localStorage.getItem("jwt_refresh_token")).toBe(mockRefreshToken);
  });

  it("should set auth state without a refresh token (legacy backend / flag off)", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };
    const mockToken = "test-jwt-token";

    useAuthStore.getState().setAuth(mockUser, mockToken);

    const state = useAuthStore.getState();
    expect(state.user).toEqual(mockUser);
    expect(state.token).toBe(mockToken);
    expect(state.refreshToken).toBeNull();
    expect(state.isAuthenticated).toBe(true);
    expect(localStorage.getItem("jwt_token")).toBe(mockToken);
    expect(localStorage.getItem("jwt_refresh_token")).toBeNull();
  });

  it("should rotate only the tokens via updateTokens without touching the user", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.getState().setAuth(mockUser, "old-access", "old-refresh");
    useAuthStore.getState().updateTokens("new-access", "new-refresh");

    const state = useAuthStore.getState();
    expect(state.user).toEqual(mockUser);
    expect(state.token).toBe("new-access");
    expect(state.refreshToken).toBe("new-refresh");
    expect(state.isAuthenticated).toBe(true);
    expect(localStorage.getItem("jwt_token")).toBe("new-access");
    expect(localStorage.getItem("jwt_refresh_token")).toBe("new-refresh");
  });

  it("should clear auth state including refresh token", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "admin",
      emailVerified: true,
      permissions: [],
    };

    // First set auth
    useAuthStore.getState().setAuth(mockUser, "token", "refresh");
    expect(useAuthStore.getState().isAuthenticated).toBe(true);

    // Then clear it
    useAuthStore.getState().clearAuth();

    const state = useAuthStore.getState();
    expect(state.user).toBeNull();
    expect(state.token).toBeNull();
    expect(state.refreshToken).toBeNull();
    expect(state.isAuthenticated).toBe(false);
    expect(localStorage.getItem("jwt_token")).toBeNull();
    expect(localStorage.getItem("jwt_refresh_token")).toBeNull();
  });

  it("should persist auth state including refresh token", () => {
    const mockUser: User = {
      id: "1",
      username: "testuser",
      email: "test@example.com",
      role: "reader",
      emailVerified: true,
      permissions: [],
    };

    useAuthStore.getState().setAuth(mockUser, "token", "refresh");

    // Check if state is stored in localStorage
    const storedData = localStorage.getItem("auth-storage");
    expect(storedData).toBeTruthy();

    const parsed = JSON.parse(storedData!);
    expect(parsed.state.user).toEqual(mockUser);
    expect(parsed.state.token).toBe("token");
    expect(parsed.state.refreshToken).toBe("refresh");
  });
});
