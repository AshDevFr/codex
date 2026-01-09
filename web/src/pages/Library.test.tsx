import { screen, waitFor } from "@testing-library/react";
import { Route, Routes } from "react-router-dom";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders } from "@/test/utils";
import { LibraryPage } from "./Library";

// Mock the API
vi.mock("@/api/libraries", () => ({
	librariesApi: {
		getById: vi.fn(),
	},
}));

// Mock the section components
vi.mock("@/components/library/RecommendedSection", () => ({
	RecommendedSection: ({ libraryId }: { libraryId: string }) => (
		<div data-testid="recommended-section">Recommended: {libraryId}</div>
	),
}));

vi.mock("@/components/library/SeriesSection", () => ({
	SeriesSection: ({ libraryId }: { libraryId: string }) => (
		<div data-testid="series-section">Series: {libraryId}</div>
	),
}));

vi.mock("@/components/library/BooksSection", () => ({
	BooksSection: ({ libraryId }: { libraryId: string }) => (
		<div data-testid="books-section">Books: {libraryId}</div>
	),
}));

const renderWithRouter = (initialPath: string) => {
	return renderWithProviders(
		<Routes>
			<Route path="/libraries/:libraryId/*" element={<LibraryPage />} />
		</Routes>,
		{ initialEntries: [initialPath] },
	);
};

describe("LibraryPage", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("should render library name for specific library", async () => {
		const mockLibrary = {
			id: "lib-123",
			name: "Test Library",
			path: "/test/path",
			isActive: true,
			createdAt: "2024-01-01",
			updatedAt: "2024-01-01",
		};

		vi.mocked(librariesApi.getById).mockResolvedValue(mockLibrary);

		renderWithRouter("/libraries/lib-123/recommended");

		await waitFor(() => {
			expect(screen.getByText("Test Library")).toBeInTheDocument();
		});
	});

	it("should render 'All Libraries' for all libraries view", async () => {
		renderWithRouter("/libraries/all/recommended");

		await waitFor(() => {
			expect(screen.getByText("All Libraries")).toBeInTheDocument();
		});
	});

	it("should render recommended tab by default", async () => {
		renderWithRouter("/libraries/all/recommended");

		await waitFor(() => {
			expect(screen.getByTestId("recommended-section")).toBeInTheDocument();
		});
	});

	it("should render series tab when navigating to series", async () => {
		renderWithRouter("/libraries/all/series");

		await waitFor(() => {
			expect(screen.getByTestId("series-section")).toBeInTheDocument();
		});
	});

	it("should render books tab when navigating to books", async () => {
		renderWithRouter("/libraries/all/books");

		await waitFor(() => {
			expect(screen.getByTestId("books-section")).toBeInTheDocument();
		});
	});

	it("should not fetch library data for all libraries view", async () => {
		renderWithRouter("/libraries/all/recommended");

		await waitFor(() => {
			expect(screen.getByText("All Libraries")).toBeInTheDocument();
		});

		expect(librariesApi.getById).not.toHaveBeenCalled();
	});

	it("should fetch library data for specific library", async () => {
		const mockLibrary = {
			id: "lib-123",
			name: "Test Library",
			path: "/test/path",
			isActive: true,
			createdAt: "2024-01-01",
			updatedAt: "2024-01-01",
		};

		vi.mocked(librariesApi.getById).mockResolvedValue(mockLibrary);

		renderWithRouter("/libraries/lib-123/recommended");

		await waitFor(() => {
			expect(librariesApi.getById).toHaveBeenCalledWith("lib-123");
		});
	});
});
