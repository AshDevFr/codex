import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders } from "@/test/utils";
import { PreviewScanPanel } from "./PreviewScanPanel";

// Mock the libraries API
vi.mock("@/api/libraries", () => ({
	librariesApi: {
		previewScan: vi.fn(),
	},
}));

describe("PreviewScanPanel", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("shows warning when path is empty", () => {
		renderWithProviders(
			<PreviewScanPanel
				path=""
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		expect(
			screen.getByText(/select a library path first/i),
		).toBeInTheDocument();
	});

	it("shows preview button when path is provided", () => {
		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		expect(
			screen.getByRole("button", { name: /preview/i }),
		).toBeInTheDocument();
	});

	it("shows placeholder when no scan has been performed", () => {
		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		expect(
			screen.getByText(/click "preview" to see how your folder structure/i),
		).toBeInTheDocument();
	});

	it("calls previewScan API when preview button is clicked", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockResolvedValue({
			detectedSeries: [],
			totalFiles: 0,
		});

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		expect(mockPreviewScan).toHaveBeenCalledWith({
			path: "/media/comics",
			seriesStrategy: "series_volume",
			seriesConfig: undefined,
		});
	});

	it("shows loading state while scanning", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);

		// Create a promise that we can control
		let resolvePromise: (value: {
			detectedSeries: [];
			totalFiles: number;
		}) => void;
		mockPreviewScan.mockImplementation(
			() =>
				new Promise((resolve) => {
					resolvePromise = resolve;
				}),
		);

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		expect(screen.getByText(/scanning folder structure/i)).toBeInTheDocument();

		// Resolve the promise
		resolvePromise?.({ detectedSeries: [], totalFiles: 0 });

		await waitFor(() => {
			expect(
				screen.queryByText(/scanning folder structure/i),
			).not.toBeInTheDocument();
		});
	});

	it("displays detected series after scan", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockResolvedValue({
			detectedSeries: [
				{
					name: "Batman",
					path: "/media/comics/Batman",
					bookCount: 10,
					sampleBooks: ["Batman #001.cbz", "Batman #002.cbz"],
				},
				{
					name: "Spider-Man",
					path: "/media/comics/Spider-Man",
					bookCount: 5,
					sampleBooks: ["Spider-Man #001.cbz"],
				},
			],
			totalFiles: 15,
		});

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		await waitFor(() => {
			expect(screen.getByText("Batman")).toBeInTheDocument();
			expect(screen.getByText("Spider-Man")).toBeInTheDocument();
		});

		expect(screen.getByText("2 series detected")).toBeInTheDocument();
		expect(screen.getByText("15 files found")).toBeInTheDocument();
	});

	it("shows sample books in series cards", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockResolvedValue({
			detectedSeries: [
				{
					name: "Batman",
					path: "/media/comics/Batman",
					bookCount: 10,
					sampleBooks: [
						"Batman #001.cbz",
						"Batman #002.cbz",
						"Batman #003.cbz",
					],
				},
			],
			totalFiles: 10,
		});

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		await waitFor(() => {
			expect(screen.getByText("Batman #001.cbz")).toBeInTheDocument();
			expect(screen.getByText("Batman #002.cbz")).toBeInTheDocument();
			expect(screen.getByText("Batman #003.cbz")).toBeInTheDocument();
		});

		// Should show "and X more" when there are more books
		expect(screen.getByText(/and 7 more/i)).toBeInTheDocument();
	});

	it("shows warning when no series detected", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockResolvedValue({
			detectedSeries: [],
			totalFiles: 5,
		});

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		await waitFor(() => {
			expect(screen.getByText(/no series detected/i)).toBeInTheDocument();
		});
	});

	it("shows error message on API failure", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockRejectedValue(new Error("Permission denied"));

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		await waitFor(() => {
			expect(screen.getByText("Permission denied")).toBeInTheDocument();
		});
	});

	it("passes series config to API when provided", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockResolvedValue({
			detectedSeries: [],
			totalFiles: 0,
		});

		const seriesConfig = { skipDepth: 2, storeSkippedAs: "publisher" };

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="publisher_hierarchy"
				seriesConfig={seriesConfig}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		expect(mockPreviewScan).toHaveBeenCalledWith({
			path: "/media/comics",
			seriesStrategy: "publisher_hierarchy",
			seriesConfig,
		});
	});

	it("shows rescan button after initial scan", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockResolvedValue({
			detectedSeries: [],
			totalFiles: 0,
		});

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		await waitFor(() => {
			expect(
				screen.getByRole("button", { name: /rescan/i }),
			).toBeInTheDocument();
		});
	});

	it("calls onScanComplete callback when scan finishes", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		const scanResult = {
			detectedSeries: [
				{
					name: "Test Series",
					path: "/test",
					bookCount: 1,
					sampleBooks: [],
				},
			],
			totalFiles: 1,
		};
		mockPreviewScan.mockResolvedValue(scanResult);

		const onScanComplete = vi.fn();

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="series_volume"
				seriesConfig={{}}
				onScanComplete={onScanComplete}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		await waitFor(() => {
			expect(onScanComplete).toHaveBeenCalledWith(scanResult);
		});
	});

	it("displays metadata badges when series has metadata", async () => {
		const user = userEvent.setup();
		const mockPreviewScan = vi.mocked(librariesApi.previewScan);
		mockPreviewScan.mockResolvedValue({
			detectedSeries: [
				{
					name: "Spider-Man",
					path: "/media/comics/Marvel/Spider-Man",
					bookCount: 5,
					sampleBooks: [],
					metadata: {
						publisher: "Marvel",
						author: "Stan Lee",
					},
				},
			],
			totalFiles: 5,
		});

		renderWithProviders(
			<PreviewScanPanel
				path="/media/comics"
				seriesStrategy="publisher_hierarchy"
				seriesConfig={{}}
			/>,
		);

		await user.click(screen.getByRole("button", { name: /preview/i }));

		await waitFor(() => {
			expect(screen.getByText("Marvel")).toBeInTheDocument();
			expect(screen.getByText("Stan Lee")).toBeInTheDocument();
		});
	});
});
