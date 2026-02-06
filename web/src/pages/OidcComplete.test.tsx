import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders } from "@/test/utils";
import { OidcComplete } from "./OidcComplete";

const mockNavigate = vi.fn();

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

function encodeAuthData(data: Record<string, unknown>): string {
  const json = JSON.stringify(data);
  // Convert to URL-safe base64 (matching backend encoding)
  return btoa(json).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

const mockAuthData = {
  accessToken: "oidc-jwt-token-123",
  tokenType: "Bearer",
  expiresIn: 86400,
  user: {
    id: "user-uuid-123",
    username: "johndoe",
    email: "john@example.com",
    role: "reader",
    emailVerified: true,
    permissions: [],
  },
  newAccount: false,
  provider: "authentik",
};

describe("OidcComplete Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    useAuthStore.getState().clearAuth();

    // Reset window.location.hash
    window.location.hash = "";

    // Mock window.history.replaceState
    vi.spyOn(window.history, "replaceState").mockImplementation(() => {});
  });

  it("should show loading state", () => {
    renderWithProviders(<OidcComplete />);

    expect(screen.getByText(/completing sign in/i)).toBeInTheDocument();
  });

  it("should process valid auth data from URL fragment", async () => {
    const encoded = encodeAuthData(mockAuthData);
    window.location.hash = `#${encoded}`;

    renderWithProviders(<OidcComplete />);

    await waitFor(() => {
      // Should store auth data
      expect(localStorage.getItem("jwt_token")).toBe("oidc-jwt-token-123");
      // Should clear the URL fragment
      expect(window.history.replaceState).toHaveBeenCalledWith(null, "", "/");
      // Should navigate to home
      expect(mockNavigate).toHaveBeenCalledWith("/", { replace: true });
    });
  });

  it("should store user info in auth store", async () => {
    const encoded = encodeAuthData(mockAuthData);
    window.location.hash = `#${encoded}`;

    renderWithProviders(<OidcComplete />);

    await waitFor(() => {
      const state = useAuthStore.getState();
      expect(state.isAuthenticated).toBe(true);
      expect(state.user?.username).toBe("johndoe");
      expect(state.user?.email).toBe("john@example.com");
      expect(state.token).toBe("oidc-jwt-token-123");
    });
  });

  it("should redirect to login with error when no fragment data", async () => {
    window.location.hash = "";

    renderWithProviders(<OidcComplete />);

    await waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith(
        expect.stringContaining("/login?error="),
        { replace: true },
      );
    });
  });

  it("should redirect to login with error when fragment is invalid", async () => {
    window.location.hash = "#not-valid-base64!!!";

    renderWithProviders(<OidcComplete />);

    await waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith(
        expect.stringContaining("/login?error="),
        { replace: true },
      );
    });
  });

  it("should handle new account flag", async () => {
    const newAccountData = { ...mockAuthData, newAccount: true };
    const encoded = encodeAuthData(newAccountData);
    window.location.hash = `#${encoded}`;

    renderWithProviders(<OidcComplete />);

    await waitFor(() => {
      // Should still store auth and navigate home
      expect(localStorage.getItem("jwt_token")).toBe("oidc-jwt-token-123");
      expect(mockNavigate).toHaveBeenCalledWith("/", { replace: true });
    });
  });
});
