import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { GenreTagChips } from "./GenreTagChips";

// Mock useNavigate
const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
	const actual = await vi.importActual("react-router-dom");
	return {
		...actual,
		useNavigate: () => mockNavigate,
	};
});

describe("GenreTagChips", () => {
	beforeEach(() => {
		mockNavigate.mockClear();
	});

	const mockGenres = [
		{
			id: "genre-1",
			name: "Action",
			seriesCount: 10,
			createdAt: "2024-01-01T00:00:00Z",
		},
		{
			id: "genre-2",
			name: "Adventure",
			seriesCount: 5,
			createdAt: "2024-01-01T00:00:00Z",
		},
		{
			id: "genre-3",
			name: "Comedy",
			seriesCount: 8,
			createdAt: "2024-01-01T00:00:00Z",
		},
	];

	const mockTags = [
		{
			id: "tag-1",
			name: "Favorite",
			seriesCount: 15,
			createdAt: "2024-01-01T00:00:00Z",
		},
		{
			id: "tag-2",
			name: "Completed",
			seriesCount: 20,
			createdAt: "2024-01-01T00:00:00Z",
		},
	];

	it("should render nothing when no genres or tags", () => {
		renderWithProviders(<GenreTagChips />);

		// No badges should be rendered
		expect(screen.queryByRole("group")).not.toBeInTheDocument();
	});

	it("should render genres as blue badges", () => {
		renderWithProviders(<GenreTagChips genres={mockGenres} />);

		expect(screen.getByText("Action")).toBeInTheDocument();
		expect(screen.getByText("Adventure")).toBeInTheDocument();
		expect(screen.getByText("Comedy")).toBeInTheDocument();
	});

	it("should render tags as gray badges", () => {
		renderWithProviders(<GenreTagChips tags={mockTags} />);

		expect(screen.getByText("Favorite")).toBeInTheDocument();
		expect(screen.getByText("Completed")).toBeInTheDocument();
	});

	it("should render both genres and tags together", () => {
		renderWithProviders(<GenreTagChips genres={mockGenres} tags={mockTags} />);

		expect(screen.getByText("Action")).toBeInTheDocument();
		expect(screen.getByText("Favorite")).toBeInTheDocument();
	});

	it("should navigate on genre click when clickable", async () => {
		const user = userEvent.setup();

		renderWithProviders(<GenreTagChips genres={mockGenres} clickable={true} />);

		await user.click(screen.getByText("Action"));

		expect(mockNavigate).toHaveBeenCalledWith(
			"/libraries/all/series?gf=any:Action",
		);
	});

	it("should navigate with libraryId in path when provided", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<GenreTagChips
				genres={mockGenres}
				libraryId="lib-123"
				clickable={true}
			/>,
		);

		await user.click(screen.getByText("Action"));

		expect(mockNavigate).toHaveBeenCalledWith(
			"/libraries/lib-123/series?gf=any:Action",
		);
	});

	it("should navigate on tag click when clickable", async () => {
		const user = userEvent.setup();

		renderWithProviders(<GenreTagChips tags={mockTags} clickable={true} />);

		await user.click(screen.getByText("Favorite"));

		expect(mockNavigate).toHaveBeenCalledWith(
			"/libraries/all/series?tf=any:Favorite",
		);
	});

	it("should not navigate when clickable is false", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<GenreTagChips genres={mockGenres} tags={mockTags} clickable={false} />,
		);

		await user.click(screen.getByText("Action"));
		await user.click(screen.getByText("Favorite"));

		expect(mockNavigate).not.toHaveBeenCalled();
	});

	it("should limit display items with maxDisplay", () => {
		renderWithProviders(
			<GenreTagChips genres={mockGenres} tags={mockTags} maxDisplay={3} />,
		);

		// Should show first 3 items (all genres in this case)
		expect(screen.getByText("Action")).toBeInTheDocument();
		expect(screen.getByText("Adventure")).toBeInTheDocument();
		expect(screen.getByText("Comedy")).toBeInTheDocument();

		// Tags should be hidden
		expect(screen.queryByText("Favorite")).not.toBeInTheDocument();

		// Should show "+2 more"
		expect(screen.getByText("+2 more")).toBeInTheDocument();
	});

	it("should encode special characters in genre name for URL", async () => {
		const user = userEvent.setup();
		const genresWithSpecialChars = [
			{
				id: "genre-1",
				name: "Sci-Fi & Fantasy",
				seriesCount: 5,
				createdAt: "2024-01-01T00:00:00Z",
			},
		];

		renderWithProviders(
			<GenreTagChips genres={genresWithSpecialChars} clickable={true} />,
		);

		await user.click(screen.getByText("Sci-Fi & Fantasy"));

		expect(mockNavigate).toHaveBeenCalledWith(
			"/libraries/all/series?gf=any:Sci-Fi%20%26%20Fantasy",
		);
	});

	it("should show tags when genres don't fill maxDisplay", () => {
		const singleGenre = [
			{
				id: "genre-1",
				name: "Action",
				seriesCount: 10,
				createdAt: "2024-01-01T00:00:00Z",
			},
		];

		renderWithProviders(
			<GenreTagChips genres={singleGenre} tags={mockTags} maxDisplay={3} />,
		);

		// Should show 1 genre and 2 tags
		expect(screen.getByText("Action")).toBeInTheDocument();
		expect(screen.getByText("Favorite")).toBeInTheDocument();
		expect(screen.getByText("Completed")).toBeInTheDocument();
	});

	it("should not show '+X more' when all items are displayed", () => {
		const singleGenre = [
			{
				id: "genre-1",
				name: "Action",
				seriesCount: 10,
				createdAt: "2024-01-01T00:00:00Z",
			},
		];
		const singleTag = [
			{
				id: "tag-1",
				name: "Favorite",
				seriesCount: 15,
				createdAt: "2024-01-01T00:00:00Z",
			},
		];

		renderWithProviders(
			<GenreTagChips genres={singleGenre} tags={singleTag} maxDisplay={5} />,
		);

		expect(screen.queryByText(/more/)).not.toBeInTheDocument();
	});

	it("should call navigate when badge is clicked and clickable is true", async () => {
		const user = userEvent.setup();
		renderWithProviders(<GenreTagChips genres={mockGenres} clickable={true} />);

		// Clicking a badge should trigger navigation
		await user.click(screen.getByText("Adventure"));
		expect(mockNavigate).toHaveBeenCalledWith(
			"/libraries/all/series?gf=any:Adventure",
		);
	});
});
