/**
 * Auth API mock handlers
 */

import { http, HttpResponse, delay } from "msw";
import { createUser } from "../data/factories";

// Mock user state
let currentUser = createUser({ username: "admin", isAdmin: true });
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
        { status: 400 }
      );
    }

    // Mock successful login
    isAuthenticated = true;
    currentUser = createUser({
      username: body.username,
      isAdmin: body.username === "admin",
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
        { status: 400 }
      );
    }

    const newUser = createUser({
      username: body.username,
      email: body.email,
      isAdmin: false,
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
export const setMockAuthState = (authenticated: boolean, user?: typeof currentUser) => {
  isAuthenticated = authenticated;
  if (user) {
    currentUser = user;
  }
};
