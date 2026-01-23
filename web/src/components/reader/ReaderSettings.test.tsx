import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { ReaderSettings } from "./ReaderSettings";

// Reset store before each test
beforeEach(() => {
	useReaderStore.setState({
		settings: {
			...useReaderStore.getState().settings,
			pdfMode: "streaming",
		},
	});
});

describe("ReaderSettings", () => {
	describe("PDF Mode Toggle", () => {
		it("should show PDF mode toggle when format is PDF", () => {
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			expect(screen.getByText("PDF Rendering Mode")).toBeInTheDocument();
			expect(screen.getByText("Auto")).toBeInTheDocument();
			expect(screen.getByText("Streaming")).toBeInTheDocument();
			expect(screen.getByText("Native")).toBeInTheDocument();
		});

		it("should show PDF mode toggle for lowercase pdf format", () => {
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="pdf" />,
			);

			expect(screen.getByText("PDF Rendering Mode")).toBeInTheDocument();
		});

		it("should not show PDF mode toggle for CBZ format", () => {
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="CBZ" />,
			);

			expect(screen.queryByText("PDF Rendering Mode")).not.toBeInTheDocument();
		});

		it("should not show PDF mode toggle for EPUB format", () => {
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="EPUB" />,
			);

			expect(screen.queryByText("PDF Rendering Mode")).not.toBeInTheDocument();
		});

		it("should not show PDF mode toggle when format is undefined", () => {
			renderWithProviders(<ReaderSettings opened={true} onClose={vi.fn()} />);

			expect(screen.queryByText("PDF Rendering Mode")).not.toBeInTheDocument();
		});

		it("should change PDF mode when Native is selected", async () => {
			const user = userEvent.setup();
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			// Initially streaming mode
			expect(useReaderStore.getState().settings.pdfMode).toBe("streaming");

			// Click on Native option
			await user.click(screen.getByText("Native"));

			// Should have changed to native mode
			expect(useReaderStore.getState().settings.pdfMode).toBe("native");
		});

		it("should change PDF mode when Streaming is selected", async () => {
			// Set initial state to native
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pdfMode: "native",
				},
			});

			const user = userEvent.setup();
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			// Initially native mode
			expect(useReaderStore.getState().settings.pdfMode).toBe("native");

			// Click on Streaming option
			await user.click(screen.getByText("Streaming"));

			// Should have changed to streaming mode
			expect(useReaderStore.getState().settings.pdfMode).toBe("streaming");
		});

		it("should show streaming mode description when streaming is selected", () => {
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			expect(
				screen.getByText("Server renders pages as images (lower bandwidth)"),
			).toBeInTheDocument();
		});

		it("should show native mode description when native is selected", async () => {
			const user = userEvent.setup();
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			// Switch to native mode
			await user.click(screen.getByText("Native"));

			expect(
				screen.getByText("Downloads full PDF for text selection and search"),
			).toBeInTheDocument();
		});

		it("should change PDF mode when Auto is selected", async () => {
			const user = userEvent.setup();
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			// Click on Auto option
			await user.click(screen.getByText("Auto"));

			// Should have changed to auto mode
			expect(useReaderStore.getState().settings.pdfMode).toBe("auto");
		});

		it("should show auto mode description when auto is selected", async () => {
			const user = userEvent.setup();
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			// Switch to auto mode
			await user.click(screen.getByText("Auto"));

			expect(
				screen.getByText(
					"Automatically selects based on file size (>100MB uses streaming)",
				),
			).toBeInTheDocument();
		});

		it("should show re-open warning message", () => {
			renderWithProviders(
				<ReaderSettings opened={true} onClose={vi.fn()} format="PDF" />,
			);

			expect(
				screen.getByText("Re-open the book after changing to apply"),
			).toBeInTheDocument();
		});
	});

	describe("Modal Behavior", () => {
		it("should render modal when opened is true", () => {
			renderWithProviders(<ReaderSettings opened={true} onClose={vi.fn()} />);

			expect(screen.getByText("Reader Settings")).toBeInTheDocument();
		});

		it("should not render modal content when opened is false", () => {
			renderWithProviders(<ReaderSettings opened={false} onClose={vi.fn()} />);

			expect(screen.queryByText("Reader Settings")).not.toBeInTheDocument();
		});
	});
});
