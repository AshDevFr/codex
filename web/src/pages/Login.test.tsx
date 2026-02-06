import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authApi } from "@/api/auth";
import { setupApi } from "@/api/setup";
import { renderWithProviders, userEvent } from "@/test/utils";
import type {
  LoginResponse,
  OidcLoginResponse,
  OidcProvidersResponse,
  SetupStatusResponse,
} from "@/types";
import { Login } from "./Login";

vi.mock("@/api/auth");
vi.mock("@/api/setup");

const mockNavigate = vi.fn();
let mockSearchParams = new URLSearchParams();

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
    useSearchParams: () => [mockSearchParams],
  };
});

const mockSetupStatus = (
  registrationEnabled: boolean,
): SetupStatusResponse => ({
  setupRequired: false,
  hasUsers: true,
  registrationEnabled,
});

const mockOidcDisabled: OidcProvidersResponse = {
  enabled: false,
  providers: [],
};

const mockOidcEnabled: OidcProvidersResponse = {
  enabled: true,
  providers: [
    {
      name: "authentik",
      displayName: "Authentik SSO",
      loginUrl: "/api/v1/auth/oidc/authentik/login",
    },
  ],
};

const mockOidcMultipleProviders: OidcProvidersResponse = {
  enabled: true,
  providers: [
    {
      name: "authentik",
      displayName: "Authentik SSO",
      loginUrl: "/api/v1/auth/oidc/authentik/login",
    },
    {
      name: "keycloak",
      displayName: "Keycloak",
      loginUrl: "/api/v1/auth/oidc/keycloak/login",
    },
  ],
};

describe("Login Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mockSearchParams = new URLSearchParams();

    // Mock window.location
    delete (window as any).location;
    window.location = { href: "" } as any;

    // Default: registration enabled, OIDC disabled
    vi.mocked(setupApi.checkStatus).mockResolvedValue(mockSetupStatus(true));
    vi.mocked(authApi.getOidcProviders).mockResolvedValue(mockOidcDisabled);
  });

  it("should render login form", async () => {
    renderWithProviders(<Login />);

    expect(screen.getByText("Welcome to Codex")).toBeInTheDocument();
    expect(screen.getByLabelText(/username/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /sign in/i }),
    ).toBeInTheDocument();
  });

  it("should show create account link when registration is enabled", async () => {
    vi.mocked(setupApi.checkStatus).mockResolvedValue(mockSetupStatus(true));

    renderWithProviders(<Login />);

    await waitFor(() => {
      expect(screen.getByText(/create one/i)).toBeInTheDocument();
    });
  });

  it("should hide create account link when registration is disabled", async () => {
    vi.mocked(setupApi.checkStatus).mockResolvedValue(mockSetupStatus(false));

    renderWithProviders(<Login />);

    await waitFor(() => {
      expect(setupApi.checkStatus).toHaveBeenCalled();
    });

    expect(screen.queryByText(/create one/i)).not.toBeInTheDocument();
  });

  it("should handle successful login", async () => {
    const user = userEvent.setup();
    const mockResponse: LoginResponse = {
      accessToken: "test-token",
      tokenType: "Bearer",
      expiresIn: 3600,
      user: {
        id: "1",
        username: "testuser",
        email: "test@example.com",
        role: "reader",
        emailVerified: true,
        permissions: [],
      },
    };

    vi.mocked(authApi.login).mockResolvedValueOnce(mockResponse);

    renderWithProviders(<Login />);

    // Fill in form
    await user.type(screen.getByLabelText(/username/i), "testuser");
    await user.type(screen.getByLabelText(/password/i), "password123");

    // Submit form
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    await waitFor(() => {
      expect(authApi.login).toHaveBeenCalled();
      expect(vi.mocked(authApi.login).mock.calls[0][0]).toEqual({
        username: "testuser",
        password: "password123",
      });
    });

    await waitFor(() => {
      expect(localStorage.getItem("jwt_token")).toBe("test-token");
      expect(mockNavigate).toHaveBeenCalledWith("/");
    });
  });

  it("should show error message on login failure", async () => {
    const user = userEvent.setup();
    const mockError = {
      error: "Invalid credentials",
      message: "Username or password is incorrect",
    };

    vi.mocked(authApi.login).mockRejectedValueOnce(mockError);

    renderWithProviders(<Login />);

    await user.type(screen.getByLabelText(/username/i), "wronguser");
    await user.type(screen.getByLabelText(/password/i), "wrongpass");
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    await waitFor(() => {
      expect(screen.getByText(/invalid credentials/i)).toBeInTheDocument();
    });
  });

  it("should require username and password", async () => {
    const user = userEvent.setup();

    renderWithProviders(<Login />);

    // Try to submit without filling form
    const submitButton = screen.getByRole("button", { name: /sign in/i });
    await user.click(submitButton);

    // Form should not submit (native HTML5 validation)
    expect(authApi.login).not.toHaveBeenCalled();
  });

  it("should show loading state while submitting", async () => {
    const user = userEvent.setup();

    vi.mocked(authApi.login).mockImplementationOnce(
      () => new Promise((resolve) => setTimeout(resolve, 100)),
    );

    renderWithProviders(<Login />);

    await user.type(screen.getByLabelText(/username/i), "testuser");
    await user.type(screen.getByLabelText(/password/i), "password123");
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    // Button should show loading state
    const button = screen.getByRole("button", { name: /sign in/i });
    expect(button).toHaveAttribute("data-loading", "true");
  });

  // OIDC tests

  it("should not show OIDC buttons when OIDC is disabled", async () => {
    vi.mocked(authApi.getOidcProviders).mockResolvedValue(mockOidcDisabled);

    renderWithProviders(<Login />);

    await waitFor(() => {
      expect(authApi.getOidcProviders).toHaveBeenCalled();
    });

    expect(screen.queryByText(/sign in with/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/or continue with/i)).not.toBeInTheDocument();
  });

  it("should show OIDC provider button when OIDC is enabled", async () => {
    vi.mocked(authApi.getOidcProviders).mockResolvedValue(mockOidcEnabled);

    renderWithProviders(<Login />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /sign in with authentik sso/i }),
      ).toBeInTheDocument();
    });

    // Should show divider between OIDC and local login
    expect(screen.getByText(/or continue with/i)).toBeInTheDocument();
  });

  it("should show multiple OIDC provider buttons", async () => {
    vi.mocked(authApi.getOidcProviders).mockResolvedValue(
      mockOidcMultipleProviders,
    );

    renderWithProviders(<Login />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /sign in with authentik sso/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /sign in with keycloak/i }),
      ).toBeInTheDocument();
    });
  });

  it("should initiate OIDC login and redirect to IdP", async () => {
    const user = userEvent.setup();
    const mockOidcResponse: OidcLoginResponse = {
      redirectUrl: "https://auth.example.com/authorize?client_id=abc",
    };

    vi.mocked(authApi.getOidcProviders).mockResolvedValue(mockOidcEnabled);
    vi.mocked(authApi.initiateOidcLogin).mockResolvedValueOnce(
      mockOidcResponse,
    );

    renderWithProviders(<Login />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /sign in with authentik sso/i }),
      ).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /sign in with authentik sso/i }),
    );

    await waitFor(() => {
      expect(authApi.initiateOidcLogin).toHaveBeenCalled();
      expect(vi.mocked(authApi.initiateOidcLogin).mock.calls[0][0]).toBe(
        "authentik",
      );
      expect(window.location.href).toBe(
        "https://auth.example.com/authorize?client_id=abc",
      );
    });
  });

  it("should show OIDC error from URL search params", async () => {
    mockSearchParams = new URLSearchParams(
      "error=access_denied&error_description=User cancelled authentication",
    );

    renderWithProviders(<Login />);

    expect(
      screen.getByText(/user cancelled authentication/i),
    ).toBeInTheDocument();
  });

  it("should show OIDC error code when no description is provided", async () => {
    mockSearchParams = new URLSearchParams("error=access_denied");

    renderWithProviders(<Login />);

    expect(screen.getByText(/access_denied/i)).toBeInTheDocument();
  });

  it("should show error when OIDC login initiation fails", async () => {
    const user = userEvent.setup();
    vi.mocked(authApi.getOidcProviders).mockResolvedValue(mockOidcEnabled);
    vi.mocked(authApi.initiateOidcLogin).mockRejectedValueOnce({
      error: "Failed to start SSO login",
      message: "Provider unavailable",
    });

    renderWithProviders(<Login />);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /sign in with authentik sso/i }),
      ).toBeInTheDocument();
    });

    await user.click(
      screen.getByRole("button", { name: /sign in with authentik sso/i }),
    );

    await waitFor(() => {
      expect(
        screen.getByText(/failed to start sso login/i),
      ).toBeInTheDocument();
    });
  });
});
