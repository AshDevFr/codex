import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setupApi } from "@/api/setup";
import { renderWithProviders } from "@/test/utils";
import { Setup } from "./Setup";

vi.mock("@/api/setup");
vi.mock("@mantine/notifications", () => ({
	notifications: {
		show: vi.fn(),
	},
}));

// Mock useNavigate
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
	const actual = await vi.importActual("react-router-dom");
	return {
		...actual,
		useNavigate: () => mockNavigate,
	};
});

describe("Setup", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		// Mock setup status to return not initialized
		vi.mocked(setupApi.checkStatus).mockResolvedValue({
			setupRequired: true,
			hasUsers: false,
			registrationEnabled: false,
		});
	});

	it("should render setup form when not initialized", async () => {
		renderWithProviders(<Setup />);

		await waitFor(() => {
			expect(screen.getByText("Welcome to Codex")).toBeInTheDocument();
		});

		expect(
			screen.getByText("Let's set up your Codex instance"),
		).toBeInTheDocument();
	});
});
