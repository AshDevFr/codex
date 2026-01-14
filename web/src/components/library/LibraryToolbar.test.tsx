import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { LibraryToolbar, type SortOption } from "./LibraryToolbar";

// Series sort options with new interface
const seriesSortOptions: SortOption[] = [
	{ field: "name", label: "Name", defaultDirection: "asc" },
	{ field: "date_added", label: "Date Added", defaultDirection: "desc" },
	{ field: "date_updated", label: "Date Updated", defaultDirection: "desc" },
	{ field: "release_date", label: "Release Date", defaultDirection: "desc" },
	{ field: "date_read", label: "Recently Read", defaultDirection: "desc" },
	{ field: "book_count", label: "Book Count", defaultDirection: "desc" },
];

describe("LibraryToolbar", () => {
	const defaultProps = {
		currentTab: "series",
		onTabChange: vi.fn(),
	};

	const sortOptions: SortOption[] = [
		{ field: "name", label: "Name", defaultDirection: "asc" },
		{ field: "created_at", label: "Recently Added", defaultDirection: "desc" },
	];

	it("should render tabs without recommended when showRecommended is false", () => {
		renderWithProviders(
			<LibraryToolbar {...defaultProps} showRecommended={false} />,
		);

		expect(screen.queryByText("Recommended")).not.toBeInTheDocument();
		expect(screen.getByText("Series")).toBeInTheDocument();
		expect(screen.getByText("Books")).toBeInTheDocument();
	});

	it("should render tabs with recommended when showRecommended is true", () => {
		renderWithProviders(
			<LibraryToolbar {...defaultProps} showRecommended={true} />,
		);

		expect(screen.getByText("Recommended")).toBeInTheDocument();
		expect(screen.getByText("Series")).toBeInTheDocument();
		expect(screen.getByText("Books")).toBeInTheDocument();
	});

	it("should call onTabChange when tab is clicked", async () => {
		const user = userEvent.setup();
		const onTabChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				onTabChange={onTabChange}
				showRecommended={true}
			/>,
		);

		await user.click(screen.getByText("Books"));
		expect(onTabChange).toHaveBeenCalledWith("books");
	});

	it("should not show controls on recommended tab", () => {
		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="recommended"
				showRecommended={true}
				sortOptions={sortOptions}
			/>,
		);

		expect(screen.queryByLabelText("Sort options")).not.toBeInTheDocument();
		expect(
			screen.queryByLabelText("Page size options"),
		).not.toBeInTheDocument();
	});

	it("should show controls on series tab", () => {
		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
			/>,
		);

		expect(screen.getByLabelText("Sort options")).toBeInTheDocument();
		expect(screen.getByLabelText("Page size options")).toBeInTheDocument();
	});

	it("should show controls on books tab", () => {
		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="books"
				sortOptions={sortOptions}
			/>,
		);

		expect(screen.getByLabelText("Sort options")).toBeInTheDocument();
		expect(screen.getByLabelText("Page size options")).toBeInTheDocument();
	});

	it("should use default direction when selecting a new sort field", async () => {
		const user = userEvent.setup();
		const onSortChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
				sort="name,asc"
				onSortChange={onSortChange}
			/>,
		);

		// Click sort button to open menu
		await user.click(screen.getByLabelText("Sort options"));

		// Click a different sort option - should use its default direction (desc)
		await user.click(await screen.findByText("Recently Added"));

		expect(onSortChange).toHaveBeenCalledWith("created_at,desc");
	});

	it("should toggle direction when clicking the same sort field", async () => {
		const user = userEvent.setup();
		const onSortChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
				sort="name,asc"
				onSortChange={onSortChange}
			/>,
		);

		// Click sort button to open menu
		await user.click(screen.getByLabelText("Sort options"));

		// Click the same sort option - should toggle to desc
		await user.click(await screen.findByText("Name"));

		expect(onSortChange).toHaveBeenCalledWith("name,desc");
	});

	it("should toggle from desc to asc when clicking the same sort field", async () => {
		const user = userEvent.setup();
		const onSortChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
				sort="name,desc"
				onSortChange={onSortChange}
			/>,
		);

		// Click sort button to open menu
		await user.click(screen.getByLabelText("Sort options"));

		// Click the same sort option - should toggle to asc
		await user.click(await screen.findByText("Name"));

		expect(onSortChange).toHaveBeenCalledWith("name,asc");
	});

	it("should call onPageSizeChange when page size is selected", async () => {
		const user = userEvent.setup();
		const onPageSizeChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
				pageSize={20}
				onPageSizeChange={onPageSizeChange}
			/>,
		);

		// Click page size button to open menu
		await user.click(screen.getByLabelText("Page size options"));

		// Wait for menu to be present and click page size option
		await user.click(await screen.findByText("50"));

		expect(onPageSizeChange).toHaveBeenCalledWith(50);
	});

	it("should highlight selected sort option", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
				sort="name,desc"
				onSortChange={vi.fn()}
			/>,
		);

		// Click sort button to open menu
		await user.click(screen.getByLabelText("Sort options"));

		// The selected option should have a background color
		const selectedOption = await screen.findByText("Name");
		expect(selectedOption.parentElement).toHaveStyle({
			background: "var(--mantine-color-blue-light)",
		});
	});

	it("should highlight selected page size option", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
				pageSize={100}
				onPageSizeChange={vi.fn()}
			/>,
		);

		// Click page size button to open menu
		await user.click(screen.getByLabelText("Page size options"));

		// The selected option should have a background color
		const selectedOption = await screen.findByText("100");
		expect(selectedOption.parentElement).toHaveStyle({
			background: "var(--mantine-color-blue-light)",
		});
	});

	it("should show filter button", () => {
		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
			/>,
		);

		const filterButton = screen.getByLabelText("Filter options");
		expect(filterButton).toBeInTheDocument();
		expect(filterButton).not.toBeDisabled();
	});

	it("should render all page size options", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
				pageSize={20}
				onPageSizeChange={vi.fn()}
			/>,
		);

		// Click page size button to open menu
		await user.click(screen.getByLabelText("Page size options"));

		// Check all page size options are present
		expect(await screen.findByText("20")).toBeInTheDocument();
		expect(await screen.findByText("50")).toBeInTheDocument();
		expect(await screen.findByText("100")).toBeInTheDocument();
		expect(await screen.findByText("200")).toBeInTheDocument();
		expect(await screen.findByText("500")).toBeInTheDocument();
	});
});

describe("LibraryToolbar - Series Sort Options", () => {
	const defaultProps = {
		currentTab: "series",
		onTabChange: vi.fn(),
	};

	it("should render all series sort options", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={seriesSortOptions}
				sort="name,asc"
				onSortChange={vi.fn()}
			/>,
		);

		// Click sort button to open menu
		await user.click(screen.getByLabelText("Sort options"));

		// Check all series sort options are present
		for (const option of seriesSortOptions) {
			expect(await screen.findByText(option.label)).toBeInTheDocument();
		}
	});

	it("should call onSortChange with default direction for date_added sort", async () => {
		const user = userEvent.setup();
		const onSortChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				sortOptions={seriesSortOptions}
				sort="name,asc"
				onSortChange={onSortChange}
			/>,
		);

		await user.click(screen.getByLabelText("Sort options"));
		await user.click(await screen.findByText("Date Added"));

		expect(onSortChange).toHaveBeenCalledWith("date_added,desc");
	});

	it("should call onSortChange with default direction for date_read sort", async () => {
		const user = userEvent.setup();
		const onSortChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				sortOptions={seriesSortOptions}
				sort="name,asc"
				onSortChange={onSortChange}
			/>,
		);

		await user.click(screen.getByLabelText("Sort options"));
		await user.click(await screen.findByText("Recently Read"));

		expect(onSortChange).toHaveBeenCalledWith("date_read,desc");
	});

	it("should highlight selected sort option for new sort types", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				sortOptions={seriesSortOptions}
				sort="book_count,desc"
				onSortChange={vi.fn()}
			/>,
		);

		await user.click(screen.getByLabelText("Sort options"));

		const selectedOption = await screen.findByText("Book Count");
		expect(selectedOption.parentElement).toHaveStyle({
			background: "var(--mantine-color-blue-light)",
		});
	});

	it("should toggle direction when clicking already selected option", async () => {
		const user = userEvent.setup();
		const onSortChange = vi.fn();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				sortOptions={seriesSortOptions}
				sort="book_count,desc"
				onSortChange={onSortChange}
			/>,
		);

		await user.click(screen.getByLabelText("Sort options"));
		await user.click(await screen.findByText("Book Count"));

		// Should toggle from desc to asc
		expect(onSortChange).toHaveBeenCalledWith("book_count,asc");
	});
});
