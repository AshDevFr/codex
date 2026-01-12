import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { describe, expect, it, vi } from "vitest";
import { LibraryToolbar } from "./LibraryToolbar";

// All series sort options as defined in Library.tsx
const seriesSortOptions = [
	{ value: "name,asc", label: "Name (A-Z)" },
	{ value: "name,desc", label: "Name (Z-A)" },
	{ value: "date_added,desc", label: "Date Added (Newest)" },
	{ value: "date_added,asc", label: "Date Added (Oldest)" },
	{ value: "date_updated,desc", label: "Date Updated (Newest)" },
	{ value: "date_updated,asc", label: "Date Updated (Oldest)" },
	{ value: "release_date,desc", label: "Release Date (Newest)" },
	{ value: "release_date,asc", label: "Release Date (Oldest)" },
	{ value: "date_read,desc", label: "Recently Read" },
	{ value: "file_size,desc", label: "File Size (Largest)" },
	{ value: "file_size,asc", label: "File Size (Smallest)" },
	{ value: "page_count,desc", label: "Page Count (Most)" },
	{ value: "page_count,asc", label: "Page Count (Least)" },
	{ value: "filename,asc", label: "Filename (A-Z)" },
	{ value: "filename,desc", label: "Filename (Z-A)" },
];

describe("LibraryToolbar", () => {
	const defaultProps = {
		currentTab: "series",
		onTabChange: vi.fn(),
	};

	const sortOptions = [
		{ value: "name,asc", label: "Name (A-Z)" },
		{ value: "name,desc", label: "Name (Z-A)" },
		{ value: "created_at,desc", label: "Recently Added" },
	];

	it("should render tabs without recommended when showRecommended is false", () => {
		renderWithProviders(<LibraryToolbar {...defaultProps} showRecommended={false} />);

		expect(screen.queryByText("Recommended")).not.toBeInTheDocument();
		expect(screen.getByText("Series")).toBeInTheDocument();
		expect(screen.getByText("Books")).toBeInTheDocument();
	});

	it("should render tabs with recommended when showRecommended is true", () => {
		renderWithProviders(<LibraryToolbar {...defaultProps} showRecommended={true} />);

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
		expect(screen.queryByLabelText("Page size options")).not.toBeInTheDocument();
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

	it("should call onSortChange when sort option is selected", async () => {
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

		// Click sort option
		await user.click(await screen.findByText("Name (Z-A)"));

		expect(onSortChange).toHaveBeenCalledWith("name,desc");
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
		const selectedOption = await screen.findByText("Name (Z-A)");
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

	it("should call onSortChange with correct value for date_added sort", async () => {
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
		await user.click(await screen.findByText("Date Added (Newest)"));

		expect(onSortChange).toHaveBeenCalledWith("date_added,desc");
	});

	it("should call onSortChange with correct value for file_size sort", async () => {
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
		await user.click(await screen.findByText("File Size (Largest)"));

		expect(onSortChange).toHaveBeenCalledWith("file_size,desc");
	});

	it("should call onSortChange with correct value for page_count sort", async () => {
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
		await user.click(await screen.findByText("Page Count (Most)"));

		expect(onSortChange).toHaveBeenCalledWith("page_count,desc");
	});

	it("should call onSortChange with correct value for date_read sort", async () => {
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

	it("should call onSortChange with correct value for filename sort", async () => {
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
		await user.click(await screen.findByText("Filename (A-Z)"));

		expect(onSortChange).toHaveBeenCalledWith("filename,asc");
	});

	it("should highlight selected sort option for new sort types", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				sortOptions={seriesSortOptions}
				sort="file_size,desc"
				onSortChange={vi.fn()}
			/>,
		);

		await user.click(screen.getByLabelText("Sort options"));

		const selectedOption = await screen.findByText("File Size (Largest)");
		expect(selectedOption.parentElement).toHaveStyle({
			background: "var(--mantine-color-blue-light)",
		});
	});
});
