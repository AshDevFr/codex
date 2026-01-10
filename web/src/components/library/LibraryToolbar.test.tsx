import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { describe, expect, it, vi } from "vitest";
import { LibraryToolbar } from "./LibraryToolbar";

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

	it("should show filter button as disabled", () => {
		renderWithProviders(
			<LibraryToolbar
				{...defaultProps}
				currentTab="series"
				sortOptions={sortOptions}
			/>,
		);

		const filterButton = screen.getByLabelText("Filter options");
		expect(filterButton).toBeDisabled();
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
