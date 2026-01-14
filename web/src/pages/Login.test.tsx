import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authApi } from "@/api/auth";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { LoginResponse } from "@/types";
import { Login } from "./Login";

vi.mock("@/api/auth");

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
	const actual = await vi.importActual("react-router-dom");
	return {
		...actual,
		useNavigate: () => mockNavigate,
	};
});

describe("Login Component", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		localStorage.clear();

		// Mock window.location
		delete (window as any).location;
		window.location = { href: "" } as any;
	});

	it("should render login form", () => {
		renderWithProviders(<Login />);

		expect(screen.getByText("Welcome to Codex")).toBeInTheDocument();
		expect(screen.getByLabelText(/username/i)).toBeInTheDocument();
		expect(screen.getByLabelText(/password/i)).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /sign in/i }),
		).toBeInTheDocument();
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
				isAdmin: false,
				emailVerified: true,
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
});
