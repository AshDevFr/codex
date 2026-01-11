import { renderWithProviders, screen } from "@/test/utils";
import { describe, expect, it } from "vitest";
import { BookProgress } from "./BookProgress";

describe("BookProgress", () => {
	it("should show 'Not started' when no progress", () => {
		renderWithProviders(<BookProgress progress={null} pageCount={100} />);

		expect(screen.getByText("Not started")).toBeInTheDocument();
	});

	it("should show 'Not started' when progress is undefined", () => {
		renderWithProviders(<BookProgress progress={undefined} pageCount={100} />);

		expect(screen.getByText("Not started")).toBeInTheDocument();
	});

	it("should show 'Completed' when book is completed", () => {
		const progress = {
			current_page: 99,
			completed: true,
			completed_at: "2024-01-15T10:30:00Z",
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

		expect(screen.getByText("Completed")).toBeInTheDocument();
	});

	it("should show completion date when book is completed", () => {
		const progress = {
			current_page: 99,
			completed: true,
			completed_at: "2024-01-15T10:30:00Z",
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

		// The date format depends on locale, so just check for the 'on' prefix
		expect(screen.getByText(/^on/)).toBeInTheDocument();
	});

	it("should show progress bar when reading in progress", () => {
		const progress = {
			current_page: 49, // 0-indexed, so this is page 50
			completed: false,
			completed_at: null,
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

		// Current page should be displayed as 1-indexed (50)
		expect(screen.getByText(/Page 50 of 100/)).toBeInTheDocument();
		expect(screen.getByText(/\(50%\)/)).toBeInTheDocument();
	});

	it("should calculate percentage correctly", () => {
		const progress = {
			current_page: 24, // 0-indexed, so this is page 25
			completed: false,
			completed_at: null,
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

		expect(screen.getByText(/Page 25 of 100/)).toBeInTheDocument();
		expect(screen.getByText(/\(25%\)/)).toBeInTheDocument();
	});

	it("should handle first page (0-indexed)", () => {
		const progress = {
			current_page: 0,
			completed: false,
			completed_at: null,
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

		expect(screen.getByText(/Page 1 of 100/)).toBeInTheDocument();
		expect(screen.getByText(/\(1%\)/)).toBeInTheDocument();
	});

	it("should handle edge case of zero page count", () => {
		const progress = {
			current_page: 0,
			completed: false,
			completed_at: null,
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={0} />);

		// Should show 0% when page count is 0 to avoid division by zero
		expect(screen.getByText(/\(0%\)/)).toBeInTheDocument();
	});

	it("should render progress bar element", () => {
		const progress = {
			current_page: 49,
			completed: false,
			completed_at: null,
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

		// Check that a progress bar is rendered
		expect(document.querySelector('[role="progressbar"]')).toBeInTheDocument();
	});

	it("should not show progress bar when completed", () => {
		const progress = {
			current_page: 99,
			completed: true,
			completed_at: "2024-01-15T10:30:00Z",
		};

		renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

		expect(document.querySelector('[role="progressbar"]')).not.toBeInTheDocument();
	});
});
