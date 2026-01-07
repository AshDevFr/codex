import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { filesystemApi } from "@/api/filesystem";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { Library } from "@/types/api";
import { Home } from "./Home";

vi.mock("@/api/libraries");
vi.mock("@/api/scan", () => ({
	scanApi: {
		subscribeToProgress: vi.fn(() => () => {}), // Return cleanup function
	},
}));
vi.mock("@/api/filesystem");

const mockLibraries: Library[] = [
	{
		id: "1",
		name: "Comics",
		path: "/data/comics",
		isActive: true,
		scanningConfig: {
			enabled: true,
			scanMode: "normal",
			autoScanOnCreate: false,
			scanOnStart: false,
			purgeDeletedOnScan: false,
			cronSchedule: "0 0 * * *",
		},
		createdAt: "2024-01-01T00:00:00Z",
		updatedAt: "2024-01-01T00:00:00Z",
		bookCount: 150,
		seriesCount: 25,
		lastScannedAt: "2024-01-06T00:00:00Z",
	},
	{
		id: "2",
		name: "Manga",
		path: "/data/manga",
		isActive: true,
		scanningConfig: {
			enabled: false,
			scanMode: "normal",
			autoScanOnCreate: false,
			scanOnStart: false,
			purgeDeletedOnScan: false,
		},
		createdAt: "2024-01-01T00:00:00Z",
		updatedAt: "2024-01-01T00:00:00Z",
		bookCount: 200,
		seriesCount: 30,
	},
];

describe("Home Component", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		localStorage.clear();
		// Mock localStorage for SSE authentication (scan API checks for 'token')
		localStorage.setItem("token", "test-token");

		// Mock filesystem API (used by AddLibraryModal)
		vi.mocked(filesystemApi.getDrives).mockResolvedValue([]);
		vi.mocked(filesystemApi.browse).mockResolvedValue({
			entries: [],
			current_path: "/",
			parent_path: null,
		});
	});

	it("should show loading state", async () => {
		vi.mocked(librariesApi.getAll).mockImplementationOnce(
			() => new Promise(() => {}), // Never resolves
		);

		const { container } = renderWithProviders(<Home />);

		// Mantine Loader doesn't have progressbar role, check for the loader element
		await waitFor(() => {
			expect(container.querySelector(".mantine-Loader-root")).toBeTruthy();
		});
	});

	it("should render library grid", async () => {
		vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

		renderWithProviders(<Home />);

		await waitFor(() => {
			expect(screen.getByText("Comics")).toBeInTheDocument();
			expect(screen.getByText("Manga")).toBeInTheDocument();
		});

		expect(screen.getByText("150 books")).toBeInTheDocument();
		expect(screen.getByText("25 series")).toBeInTheDocument();
		expect(screen.getByText("200 books")).toBeInTheDocument();
		expect(screen.getByText("30 series")).toBeInTheDocument();
	});

	it("should show empty state when no libraries", async () => {
		vi.mocked(librariesApi.getAll).mockResolvedValueOnce([]);

		renderWithProviders(<Home />);

		await waitFor(() => {
			expect(screen.getByText("No libraries found")).toBeInTheDocument();
		});

		expect(
			screen.getByText("Get started by adding your first library"),
		).toBeInTheDocument();
	});

	it("should handle library scan", async () => {
		const user = userEvent.setup();
		vi.mocked(librariesApi.getAll).mockResolvedValue(mockLibraries);
		vi.mocked(librariesApi.scan).mockResolvedValueOnce(undefined);

		renderWithProviders(<Home />);

		await waitFor(() => {
			expect(screen.getByText("Comics")).toBeInTheDocument();
		});

		// Find all buttons and look for the menu trigger within the Comics card
		// The menu trigger is an ActionIcon with a dots icon
		const allButtons = screen.getAllByRole("button");
		const menuTrigger = allButtons.find((btn) => {
			const svg = btn.querySelector("svg");
			const card = btn.closest('[class*="Card-root"]');
			return svg && card && card.textContent?.includes("Comics");
		});

		if (menuTrigger) {
			await user.click(menuTrigger);

			// Wait for menu to open and find "Scan Library" menu item
			const scanMenuItem = await screen.findByText("Scan Library");
			await user.click(scanMenuItem);

			await waitFor(() => {
				expect(librariesApi.scan).toHaveBeenCalledWith("1", "normal");
			});
		} else {
			// Fallback: try finding by role if structure is different
			throw new Error("Could not find menu trigger button");
		}
	});

	it("should display scan mode badges", async () => {
		vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

		renderWithProviders(<Home />);

		await waitFor(() => {
			expect(screen.getByText("Auto")).toBeInTheDocument();
			expect(screen.getByText("Manual")).toBeInTheDocument();
		});
	});

	it("should display last scan timestamp", async () => {
		vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

		renderWithProviders(<Home />);

		await waitFor(() => {
			expect(screen.getByText(/Last scan:/)).toBeInTheDocument();
		});
	});

	it("should show Add Library button", async () => {
		vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

		renderWithProviders(<Home />);

		await waitFor(() => {
			// The Add Library button is an ActionIcon with title="Add Library"
			// It doesn't have visible text, so we query by title or role
			const addButton = screen.getByTitle("Add Library");
			expect(addButton).toBeInTheDocument();
		});
	});

	it("should open Add Library modal when button is clicked", async () => {
		const user = userEvent.setup();
		vi.mocked(librariesApi.getAll).mockResolvedValueOnce(mockLibraries);

		renderWithProviders(<Home />);

		await waitFor(() => {
			expect(screen.getByText("Comics")).toBeInTheDocument();
		});

		// The Add Library button is an ActionIcon with title="Add Library"
		const addButton = screen.getByTitle("Add Library");
		await user.click(addButton);

		await waitFor(() => {
			expect(screen.getByText("Add New Library")).toBeInTheDocument();
		});
	});

	it("should show Add Library button in empty state", async () => {
		const user = userEvent.setup();
		vi.mocked(librariesApi.getAll).mockResolvedValueOnce([]);

		renderWithProviders(<Home />);

		await waitFor(() => {
			expect(screen.getByText("No libraries found")).toBeInTheDocument();
		});

		const addButtons = screen.getAllByText("Add Library");
		expect(addButtons.length).toBeGreaterThan(0);

		await user.click(addButtons[0]);

		await waitFor(() => {
			expect(screen.getByText("Add New Library")).toBeInTheDocument();
		});
	});
});
