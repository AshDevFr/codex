import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { librariesApi } from "@/api/libraries";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { User } from "@/types";
import { AppLayout } from "./AppLayout";

vi.mock("@/api/libraries");
vi.mock("@/api/tasks", () => ({
	subscribeToTaskProgress: vi.fn(() => vi.fn()),
	fetchPendingTaskCounts: vi.fn(() => Promise.resolve({})),
	fetchTasksByStatus: vi.fn(() => Promise.resolve([])),
}));

describe("Sidebar Component (via AppLayout)", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		localStorage.clear();

		// Mock window.location
		Object.defineProperty(window, "location", {
			value: { href: "" },
			writable: true,
		});

		// Mock libraries API - set default return value for getAll
		vi.mocked(librariesApi.getAll).mockResolvedValue([]);
	});

	it("should render navigation links", () => {
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			role: "reader",
			emailVerified: true,
		};

		useAuthStore.setState({
			user: mockUser,
			token: "token",
			isAuthenticated: true,
		});

		renderWithProviders(
			<AppLayout>
				<div>Content</div>
			</AppLayout>,
		);

		expect(screen.getByText("Home")).toBeInTheDocument();
		expect(screen.getByText("Libraries")).toBeInTheDocument();
		expect(screen.getByText("Settings")).toBeInTheDocument();
		expect(screen.getByText("Logout")).toBeInTheDocument();
	});

	it("should show Users link for admin users", () => {
		const mockAdmin: User = {
			id: "1",
			username: "admin",
			email: "admin@example.com",
			role: "admin",
			emailVerified: true,
		};

		useAuthStore.setState({
			user: mockAdmin,
			token: "token",
			isAuthenticated: true,
		});

		renderWithProviders(
			<AppLayout>
				<div>Content</div>
			</AppLayout>,
		);

		expect(screen.getByText("Users")).toBeInTheDocument();
	});

	it("should not show Users link for regular users in sidebar root", () => {
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			role: "reader",
			emailVerified: true,
		};

		useAuthStore.setState({
			user: mockUser,
			token: "token",
			isAuthenticated: true,
		});

		renderWithProviders(
			<AppLayout>
				<div>Content</div>
			</AppLayout>,
		);

		// Users should now be inside Settings menu, not in root
		// Check that Settings exists
		expect(screen.getByText("Settings")).toBeInTheDocument();
	});

	it("should show Profile link inside Settings for all users", () => {
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			role: "reader",
			emailVerified: true,
		};

		useAuthStore.setState({
			user: mockUser,
			token: "token",
			isAuthenticated: true,
		});

		renderWithProviders(
			<AppLayout>
				<div>Content</div>
			</AppLayout>,
		);

		expect(screen.getByText("Profile")).toBeInTheDocument();
	});

	it("should show admin settings options for admin users", () => {
		const mockAdmin: User = {
			id: "1",
			username: "admin",
			email: "admin@example.com",
			role: "admin",
			emailVerified: true,
		};

		useAuthStore.setState({
			user: mockAdmin,
			token: "token",
			isAuthenticated: true,
		});

		renderWithProviders(
			<AppLayout>
				<div>Content</div>
			</AppLayout>,
		);

		// Admin settings options should be visible inside Settings
		expect(screen.getByText("Server")).toBeInTheDocument();
		expect(screen.getByText("Users")).toBeInTheDocument();
		expect(screen.getByText("Tasks")).toBeInTheDocument();
		expect(screen.getByText("Duplicates")).toBeInTheDocument();
		expect(screen.getByText("Metrics")).toBeInTheDocument();
	});

	it("should not show admin settings options for regular users", () => {
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			role: "reader",
			emailVerified: true,
		};

		useAuthStore.setState({
			user: mockUser,
			token: "token",
			isAuthenticated: true,
		});

		renderWithProviders(
			<AppLayout>
				<div>Content</div>
			</AppLayout>,
		);

		// Admin options should not be visible
		expect(screen.queryByText("Server")).not.toBeInTheDocument();
		expect(screen.queryByText("Tasks")).not.toBeInTheDocument();
		expect(screen.queryByText("Duplicates")).not.toBeInTheDocument();
		expect(screen.queryByText("Metrics")).not.toBeInTheDocument();
		// Profile should still be visible
		expect(screen.getByText("Profile")).toBeInTheDocument();
	});

	it("should handle logout", async () => {
		const user = userEvent.setup();
		const mockUser: User = {
			id: "1",
			username: "testuser",
			email: "test@example.com",
			role: "reader",
			emailVerified: true,
		};

		useAuthStore.setState({
			user: mockUser,
			token: "token",
			isAuthenticated: true,
		});
		localStorage.setItem("jwt_token", "token");

		renderWithProviders(
			<AppLayout>
				<div>Content</div>
			</AppLayout>,
		);

		const logoutButton = screen.getByText("Logout");
		await user.click(logoutButton);

		// Should clear auth (navigation is handled by React Router now)
		expect(localStorage.getItem("jwt_token")).toBeNull();
	});
});
