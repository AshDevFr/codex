import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";
import { ComicReaderPage } from "./ComicReaderPage";

describe("ComicReaderPage", () => {
	const defaultProps = {
		src: "/api/v1/books/book-123/pages/1",
		alt: "Page 1 of Test Book",
		fitMode: "contain" as const,
		backgroundColor: "black" as const,
	};

	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("should render with loading state initially", () => {
		renderWithProviders(<ComicReaderPage {...defaultProps} />);

		// Loader should be visible initially
		expect(screen.getByRole("img", { hidden: true })).toBeInTheDocument();
	});

	it("should display image with correct src and alt", () => {
		renderWithProviders(<ComicReaderPage {...defaultProps} />);

		const img = screen.getByRole("img", { hidden: true });
		expect(img).toHaveAttribute("src", "/api/v1/books/book-123/pages/1");
		expect(img).toHaveAttribute("alt", "Page 1 of Test Book");
	});

	it("should call onClick with correct zone when clicking left third", () => {
		const onClick = vi.fn();
		renderWithProviders(
			<ComicReaderPage {...defaultProps} onClick={onClick} />,
		);

		const container = screen.getByRole("img", { hidden: true }).parentElement;
		if (container) {
			// Mock getBoundingClientRect
			vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
				left: 0,
				width: 900,
				top: 0,
				height: 600,
				right: 900,
				bottom: 600,
				x: 0,
				y: 0,
				toJSON: () => {},
			});

			fireEvent.click(container, { clientX: 100 }); // Left third (100 < 300)
			expect(onClick).toHaveBeenCalledWith("left");
		}
	});

	it("should call onClick with correct zone when clicking center", () => {
		const onClick = vi.fn();
		renderWithProviders(
			<ComicReaderPage {...defaultProps} onClick={onClick} />,
		);

		const container = screen.getByRole("img", { hidden: true }).parentElement;
		if (container) {
			vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
				left: 0,
				width: 900,
				top: 0,
				height: 600,
				right: 900,
				bottom: 600,
				x: 0,
				y: 0,
				toJSON: () => {},
			});

			fireEvent.click(container, { clientX: 450 }); // Center (300 < 450 < 600)
			expect(onClick).toHaveBeenCalledWith("center");
		}
	});

	it("should call onClick with correct zone when clicking right third", () => {
		const onClick = vi.fn();
		renderWithProviders(
			<ComicReaderPage {...defaultProps} onClick={onClick} />,
		);

		const container = screen.getByRole("img", { hidden: true }).parentElement;
		if (container) {
			vi.spyOn(container, "getBoundingClientRect").mockReturnValue({
				left: 0,
				width: 900,
				top: 0,
				height: 600,
				right: 900,
				bottom: 600,
				x: 0,
				y: 0,
				toJSON: () => {},
			});

			fireEvent.click(container, { clientX: 800 }); // Right third (800 > 600)
			expect(onClick).toHaveBeenCalledWith("right");
		}
	});

	it("should not call onClick when no handler provided", () => {
		renderWithProviders(<ComicReaderPage {...defaultProps} />);

		const container = screen.getByRole("img", { hidden: true }).parentElement;
		if (container) {
			// Should not throw
			fireEvent.click(container);
		}
	});

	it("should not render when isVisible is false", () => {
		renderWithProviders(
			<ComicReaderPage {...defaultProps} isVisible={false} />,
		);

		expect(screen.queryByRole("img")).not.toBeInTheDocument();
	});

	describe("fit modes", () => {
		it("should apply contain fit mode styles", () => {
			renderWithProviders(
				<ComicReaderPage {...defaultProps} fitMode="contain" />,
			);

			const img = screen.getByRole("img", { hidden: true });
			expect(img).toHaveStyle({ maxWidth: "100%", maxHeight: "100%" });
		});

		it("should apply width fit mode styles", () => {
			renderWithProviders(
				<ComicReaderPage {...defaultProps} fitMode="width" />,
			);

			const img = screen.getByRole("img", { hidden: true });
			expect(img).toHaveStyle({ width: "100%" });
		});

		it("should apply height fit mode styles", () => {
			renderWithProviders(
				<ComicReaderPage {...defaultProps} fitMode="height" />,
			);

			const img = screen.getByRole("img", { hidden: true });
			expect(img).toHaveStyle({ height: "100%" });
		});
	});

	describe("background colors", () => {
		it("should apply black background", () => {
			renderWithProviders(
				<ComicReaderPage {...defaultProps} backgroundColor="black" />,
			);

			const container = screen.getByRole("img", { hidden: true }).parentElement;
			expect(container).toHaveStyle({ backgroundColor: "#000000" });
		});

		it("should apply gray background", () => {
			renderWithProviders(
				<ComicReaderPage {...defaultProps} backgroundColor="gray" />,
			);

			const container = screen.getByRole("img", { hidden: true }).parentElement;
			expect(container).toHaveStyle({ backgroundColor: "#1a1a1a" });
		});

		it("should apply white background", () => {
			renderWithProviders(
				<ComicReaderPage {...defaultProps} backgroundColor="white" />,
			);

			const container = screen.getByRole("img", { hidden: true }).parentElement;
			expect(container).toHaveStyle({ backgroundColor: "#ffffff" });
		});
	});

	describe("image loading", () => {
		it("should hide loader when image loads", async () => {
			renderWithProviders(<ComicReaderPage {...defaultProps} />);

			const img = screen.getByRole("img", { hidden: true });
			fireEvent.load(img);

			await waitFor(() => {
				expect(img).not.toHaveStyle({ display: "none" });
			});
		});

		it("should show error message when image fails to load", async () => {
			renderWithProviders(<ComicReaderPage {...defaultProps} />);

			const img = screen.getByRole("img", { hidden: true });
			fireEvent.error(img);

			await waitFor(() => {
				expect(screen.getByText("Failed to load page")).toBeInTheDocument();
			});
		});
	});
});
