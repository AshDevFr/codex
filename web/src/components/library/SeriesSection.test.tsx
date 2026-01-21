import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { SeriesSection } from "./SeriesSection";

// Mock the API - never resolves to keep loading state
vi.mock("@/api/series", () => ({
	seriesApi: {
		search: vi.fn(() => new Promise(() => {})),
	},
}));

// Mock useFilterState hook
vi.mock("@/hooks/useFilterState", () => ({
	useFilterState: () => ({
		condition: undefined,
		hasActiveFilters: false,
		activeFilterCount: 0,
		filters: {
			genres: { mode: "anyOf", values: new Map() },
			tags: { mode: "anyOf", values: new Map() },
			status: { mode: "anyOf", values: new Map() },
			readStatus: { mode: "anyOf", values: new Map() },
			publisher: { mode: "anyOf", values: new Map() },
			language: { mode: "anyOf", values: new Map() },
			sharingTags: { mode: "anyOf", values: new Map() },
		},
		setGenreState: vi.fn(),
		setGenreMode: vi.fn(),
		setTagState: vi.fn(),
		setTagMode: vi.fn(),
		setStatusState: vi.fn(),
		setStatusMode: vi.fn(),
		setReadStatusState: vi.fn(),
		setReadStatusMode: vi.fn(),
		setPublisherState: vi.fn(),
		setPublisherMode: vi.fn(),
		setLanguageState: vi.fn(),
		setLanguageMode: vi.fn(),
		setSharingTagState: vi.fn(),
		setSharingTagMode: vi.fn(),
		clearAll: vi.fn(),
		clearGroup: vi.fn(),
		activeFiltersByGroup: {},
	}),
}));

describe("SeriesSection", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	const renderComponent = (searchParams = new URLSearchParams()) => {
		return renderWithProviders(
			<SeriesSection libraryId="test-library-id" searchParams={searchParams} />,
		);
	};

	describe("loading state", () => {
		it("should render skeleton placeholders while loading", () => {
			const { container } = renderComponent();

			// Should show skeleton elements while loading
			// Mantine Skeleton renders divs with mantine-Skeleton-root class
			const skeletons = container.querySelectorAll(".mantine-Skeleton-root");
			expect(skeletons.length).toBeGreaterThan(0);
		});

		it("should render correct number of skeleton items based on pageSize", () => {
			const searchParams = new URLSearchParams({ pageSize: "6" });
			const { container } = renderComponent(searchParams);

			// Count skeleton elements
			// With pageSize=6, should have 6 skeleton boxes with 2 skeletons each
			const skeletonContainers = container.querySelectorAll(
				".mantine-Skeleton-root",
			);
			// Each item has 2 skeletons (image + title), so 6 items = 12 skeletons
			expect(skeletonContainers.length).toBe(12);
		});

		it("should cap skeleton count at 12 even for larger page sizes", () => {
			const searchParams = new URLSearchParams({ pageSize: "50" });
			const { container } = renderComponent(searchParams);

			// Even with pageSize=50, should cap at 12 skeleton items (24 skeleton elements)
			const skeletonContainers = container.querySelectorAll(
				".mantine-Skeleton-root",
			);
			expect(skeletonContainers.length).toBe(24); // 12 items * 2 skeletons each
		});
	});
});
