import { beforeEach, describe, expect, it } from "vitest";
import type { User } from "@/types/api";
import { useAuthStore } from "./authStore";

describe("authStore", () => {
	beforeEach(() => {
		// Reset store state before each test
		useAuthStore.setState({
			user: null,
			token: null,
			isAuthenticated: false,
		});
		localStorage.clear();
	});

	it("should have initial state", () => {
		const state = useAuthStore.getState();
		expect(state.user).toBeNull();
		expect(state.token).toBeNull();
		expect(state.isAuthenticated).toBe(false);
	});

	it("should set auth state with user and token", () => {
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			isAdmin: false,
			emailVerified: true,
		};
		const mockToken = "test-jwt-token";

		useAuthStore.getState().setAuth(mockUser, mockToken);

		const state = useAuthStore.getState();
		expect(state.user).toEqual(mockUser);
		expect(state.token).toBe(mockToken);
		expect(state.isAuthenticated).toBe(true);
		expect(localStorage.getItem("jwt_token")).toBe(mockToken);
	});

	it("should clear auth state", () => {
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			isAdmin: true,
			emailVerified: true,
		};

		// First set auth
		useAuthStore.getState().setAuth(mockUser, "token");
		expect(useAuthStore.getState().isAuthenticated).toBe(true);

		// Then clear it
		useAuthStore.getState().clearAuth();

		const state = useAuthStore.getState();
		expect(state.user).toBeNull();
		expect(state.token).toBeNull();
		expect(state.isAuthenticated).toBe(false);
		expect(localStorage.getItem("jwt_token")).toBeNull();
	});

	it("should persist auth state", () => {
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			isAdmin: false,
			emailVerified: true,
		};

		useAuthStore.getState().setAuth(mockUser, "token");

		// Check if state is stored in localStorage
		const storedData = localStorage.getItem("auth-storage");
		expect(storedData).toBeTruthy();

		const parsed = JSON.parse(storedData!);
		expect(parsed.state.user).toEqual(mockUser);
		expect(parsed.state.token).toBe("token");
	});
});
