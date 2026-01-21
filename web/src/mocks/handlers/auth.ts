/**
 * Auth API mock handlers
 */

import { faker } from "@faker-js/faker";
import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";

type UserInfo = components["schemas"]["UserInfo"];

// Helper to create a UserInfo object (used in login/register responses)
const createUserInfo = (
	overrides: Partial<UserInfo> = {},
): UserInfo => ({
	id: faker.string.uuid(),
	username: faker.internet.username(),
	email: faker.internet.email(),
	role: "reader",
	emailVerified: true,
	...overrides,
});

// Mock user state
let currentUser: UserInfo = createUserInfo({ username: "admin", role: "admin" });
let isAuthenticated = false;

export const authHandlers = [
	// Login
	http.post("/api/v1/auth/login", async ({ request }) => {
		await delay(300);
		const body = (await request.json()) as {
			username: string;
			password: string;
		};

		// Simple validation
		if (!body.username || !body.password) {
			return HttpResponse.json(
				{ error: "Username and password required" },
				{ status: 400 },
			);
		}

		// Mock successful login
		isAuthenticated = true;
		currentUser = createUserInfo({
			username: body.username,
			email: `${body.username}@example.com`,
			role: body.username === "admin" ? "admin" : "reader",
		});

		return HttpResponse.json({
			accessToken:
				"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ",
			tokenType: "Bearer",
			expiresIn: 86400,
			user: currentUser,
		});
	}),

	// Logout
	http.post("/api/v1/auth/logout", async () => {
		await delay(100);
		isAuthenticated = false;
		return HttpResponse.json({ message: "Logged out successfully" });
	}),

	// Register
	http.post("/api/v1/auth/register", async ({ request }) => {
		await delay(500);
		const body = (await request.json()) as {
			username: string;
			email: string;
			password: string;
		};

		if (!body.username || !body.email || !body.password) {
			return HttpResponse.json(
				{ error: "All fields are required" },
				{ status: 400 },
			);
		}

		const newUser = createUserInfo({
			username: body.username,
			email: body.email,
			role: "reader",
		});

		return HttpResponse.json({
			accessToken:
				"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ",
			tokenType: "Bearer",
			expiresIn: 86400,
			user: newUser,
			message: null,
		});
	}),

	// Get current user
	http.get("/api/v1/users/me", async () => {
		await delay(100);
		if (!isAuthenticated) {
			return HttpResponse.json({ error: "Unauthorized" }, { status: 401 });
		}
		return HttpResponse.json(currentUser);
	}),

	// Verify email
	http.post("/api/v1/auth/verify-email", async ({ request }) => {
		await delay(300);
		const body = (await request.json()) as { token: string };

		if (!body.token) {
			return HttpResponse.json({ error: "Token required" }, { status: 400 });
		}

		return HttpResponse.json({
			message: "Email verified successfully",
			accessToken:
				"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ",
			tokenType: "Bearer",
			expiresIn: 86400,
			user: currentUser,
		});
	}),

	// Resend verification
	http.post("/api/v1/auth/resend-verification", async () => {
		await delay(300);
		return HttpResponse.json({ message: "Verification email sent" });
	}),
];

// Helper to set authentication state (for testing)
export const setMockAuthState = (
	authenticated: boolean,
	user?: typeof currentUser,
) => {
	isAuthenticated = authenticated;
	if (user) {
		currentUser = user;
	}
};
