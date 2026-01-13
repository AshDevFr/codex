import { describe, expect, it } from "vitest";
import {
	detectPageOrientation,
	getDisplayOrder,
	getNextSpreadPage,
	getPreloadPages,
	getPrevSpreadPage,
	getSpreadPages,
	isWidePage,
	type SpreadConfig,
} from "./spreadCalculation";

describe("spreadCalculation", () => {
	// ==========================================================================
	// detectPageOrientation
	// ==========================================================================

	describe("detectPageOrientation", () => {
		it("should return landscape when width > height", () => {
			expect(detectPageOrientation(1920, 1080)).toBe("landscape");
		});

		it("should return portrait when height > width", () => {
			expect(detectPageOrientation(1080, 1920)).toBe("portrait");
		});

		it("should return portrait when width equals height", () => {
			expect(detectPageOrientation(1000, 1000)).toBe("portrait");
		});
	});

	// ==========================================================================
	// isWidePage
	// ==========================================================================

	describe("isWidePage", () => {
		it("should return true for landscape pages", () => {
			const orientations = { 1: "landscape" as const, 2: "portrait" as const };
			expect(isWidePage(1, orientations)).toBe(true);
		});

		it("should return false for portrait pages", () => {
			const orientations = { 1: "landscape" as const, 2: "portrait" as const };
			expect(isWidePage(2, orientations)).toBe(false);
		});

		it("should return false for unknown pages", () => {
			const orientations = { 1: "landscape" as const };
			expect(isWidePage(3, orientations)).toBe(false);
		});
	});

	// ==========================================================================
	// getSpreadPages - basic behavior
	// ==========================================================================

	describe("getSpreadPages - basic behavior", () => {
		const baseConfig: SpreadConfig = {
			totalPages: 10,
			pageOrientations: {},
			showWideAlone: true,
			startOnOdd: true,
			readingDirection: "ltr",
		};

		it("should return empty for invalid page numbers", () => {
			expect(getSpreadPages(0, baseConfig)).toEqual({
				pages: [],
				isSinglePage: true,
			});
			expect(getSpreadPages(-1, baseConfig)).toEqual({
				pages: [],
				isSinglePage: true,
			});
			expect(getSpreadPages(11, baseConfig)).toEqual({
				pages: [],
				isSinglePage: true,
			});
		});

		it("should return empty for zero total pages", () => {
			const config = { ...baseConfig, totalPages: 0 };
			expect(getSpreadPages(1, config)).toEqual({
				pages: [],
				isSinglePage: true,
			});
		});
	});

	// ==========================================================================
	// getSpreadPages - startOnOdd=true (manga cover mode)
	// ==========================================================================

	describe("getSpreadPages - startOnOdd=true", () => {
		const config: SpreadConfig = {
			totalPages: 10,
			pageOrientations: {},
			showWideAlone: true,
			startOnOdd: true,
			readingDirection: "ltr",
		};

		it("should show page 1 alone", () => {
			expect(getSpreadPages(1, config)).toEqual({
				pages: [1],
				isSinglePage: true,
			});
		});

		it("should show pages 2-3 as a spread when on page 2", () => {
			expect(getSpreadPages(2, config)).toEqual({
				pages: [2, 3],
				isSinglePage: false,
			});
		});

		it("should show pages 2-3 as a spread when on page 3", () => {
			expect(getSpreadPages(3, config)).toEqual({
				pages: [2, 3],
				isSinglePage: false,
			});
		});

		it("should show pages 4-5 as a spread when on page 4", () => {
			expect(getSpreadPages(4, config)).toEqual({
				pages: [4, 5],
				isSinglePage: false,
			});
		});

		it("should show pages 4-5 as a spread when on page 5", () => {
			expect(getSpreadPages(5, config)).toEqual({
				pages: [4, 5],
				isSinglePage: false,
			});
		});

		it("should show last page alone if odd number of remaining pages", () => {
			// With 10 pages: 1 alone, 2-3, 4-5, 6-7, 8-9, 10 alone
			expect(getSpreadPages(10, config)).toEqual({
				pages: [10],
				isSinglePage: true,
			});
		});

		it("should handle 9 pages correctly", () => {
			const config9 = { ...config, totalPages: 9 };
			// 1 alone, 2-3, 4-5, 6-7, 8-9
			expect(getSpreadPages(9, config9)).toEqual({
				pages: [8, 9],
				isSinglePage: false,
			});
		});
	});

	// ==========================================================================
	// getSpreadPages - startOnOdd=false (standard mode)
	// ==========================================================================

	describe("getSpreadPages - startOnOdd=false", () => {
		const config: SpreadConfig = {
			totalPages: 10,
			pageOrientations: {},
			showWideAlone: true,
			startOnOdd: false,
			readingDirection: "ltr",
		};

		it("should show pages 1-2 as a spread when on page 1", () => {
			expect(getSpreadPages(1, config)).toEqual({
				pages: [1, 2],
				isSinglePage: false,
			});
		});

		it("should show pages 1-2 as a spread when on page 2", () => {
			expect(getSpreadPages(2, config)).toEqual({
				pages: [1, 2],
				isSinglePage: false,
			});
		});

		it("should show pages 3-4 as a spread when on page 3", () => {
			expect(getSpreadPages(3, config)).toEqual({
				pages: [3, 4],
				isSinglePage: false,
			});
		});

		it("should show pages 3-4 as a spread when on page 4", () => {
			expect(getSpreadPages(4, config)).toEqual({
				pages: [3, 4],
				isSinglePage: false,
			});
		});

		it("should show pages 9-10 as a spread", () => {
			expect(getSpreadPages(9, config)).toEqual({
				pages: [9, 10],
				isSinglePage: false,
			});
		});

		it("should show last page alone with odd total pages", () => {
			const config9 = { ...config, totalPages: 9 };
			// 1-2, 3-4, 5-6, 7-8, 9 alone
			expect(getSpreadPages(9, config9)).toEqual({
				pages: [9],
				isSinglePage: true,
			});
		});
	});

	// ==========================================================================
	// getSpreadPages - wide page handling
	// ==========================================================================

	describe("getSpreadPages - wide page handling", () => {
		it("should show landscape page alone when showWideAlone is true", () => {
			const config: SpreadConfig = {
				totalPages: 10,
				pageOrientations: { 2: "landscape" },
				showWideAlone: true,
				startOnOdd: true,
				readingDirection: "ltr",
			};
			expect(getSpreadPages(2, config)).toEqual({
				pages: [2],
				isSinglePage: true,
			});
		});

		it("should show landscape page in spread when showWideAlone is false", () => {
			const config: SpreadConfig = {
				totalPages: 10,
				pageOrientations: { 2: "landscape" },
				showWideAlone: false,
				startOnOdd: true,
				readingDirection: "ltr",
			};
			expect(getSpreadPages(2, config)).toEqual({
				pages: [2, 3],
				isSinglePage: false,
			});
		});

		it("should show left page alone when right page is landscape", () => {
			const config: SpreadConfig = {
				totalPages: 10,
				pageOrientations: { 3: "landscape" },
				showWideAlone: true,
				startOnOdd: true,
				readingDirection: "ltr",
			};
			// Page 2 would pair with 3, but 3 is landscape
			expect(getSpreadPages(2, config)).toEqual({
				pages: [2],
				isSinglePage: true,
			});
		});

		it("should not affect portrait pages", () => {
			const config: SpreadConfig = {
				totalPages: 10,
				pageOrientations: { 2: "portrait", 3: "portrait" },
				showWideAlone: true,
				startOnOdd: true,
				readingDirection: "ltr",
			};
			expect(getSpreadPages(2, config)).toEqual({
				pages: [2, 3],
				isSinglePage: false,
			});
		});
	});

	// ==========================================================================
	// getDisplayOrder
	// ==========================================================================

	describe("getDisplayOrder", () => {
		it("should return pages in order for LTR", () => {
			expect(getDisplayOrder([2, 3], "ltr")).toEqual([2, 3]);
		});

		it("should reverse pages for RTL", () => {
			expect(getDisplayOrder([2, 3], "rtl")).toEqual([3, 2]);
		});

		it("should return single page as-is for LTR", () => {
			expect(getDisplayOrder([5], "ltr")).toEqual([5]);
		});

		it("should return single page as-is for RTL", () => {
			expect(getDisplayOrder([5], "rtl")).toEqual([5]);
		});

		it("should return empty array as-is", () => {
			expect(getDisplayOrder([], "ltr")).toEqual([]);
			expect(getDisplayOrder([], "rtl")).toEqual([]);
		});
	});

	// ==========================================================================
	// getNextSpreadPage
	// ==========================================================================

	describe("getNextSpreadPage", () => {
		const config: SpreadConfig = {
			totalPages: 10,
			pageOrientations: {},
			showWideAlone: true,
			startOnOdd: true,
			readingDirection: "ltr",
		};

		it("should return page 2 when on page 1 (startOnOdd)", () => {
			expect(getNextSpreadPage(1, config)).toBe(2);
		});

		it("should return page 4 when on page 2 (skip over 3)", () => {
			expect(getNextSpreadPage(2, config)).toBe(4);
		});

		it("should return page 4 when on page 3", () => {
			expect(getNextSpreadPage(3, config)).toBe(4);
		});

		it("should return null when on last spread", () => {
			expect(getNextSpreadPage(10, config)).toBe(null);
		});

		it("should return null when next would exceed total", () => {
			const config9 = { ...config, totalPages: 9 };
			// 8-9 is the last spread
			expect(getNextSpreadPage(8, config9)).toBe(null);
			expect(getNextSpreadPage(9, config9)).toBe(null);
		});

		it("should handle landscape pages correctly", () => {
			const configWithLandscape: SpreadConfig = {
				...config,
				pageOrientations: { 3: "landscape" },
			};
			// Page 2 alone (because 3 is landscape), next is 3
			expect(getNextSpreadPage(2, configWithLandscape)).toBe(3);
			// Page 3 alone (landscape), next is 4
			expect(getNextSpreadPage(3, configWithLandscape)).toBe(4);
		});
	});

	// ==========================================================================
	// getPrevSpreadPage
	// ==========================================================================

	describe("getPrevSpreadPage", () => {
		const config: SpreadConfig = {
			totalPages: 10,
			pageOrientations: {},
			showWideAlone: true,
			startOnOdd: true,
			readingDirection: "ltr",
		};

		it("should return null when on page 1", () => {
			expect(getPrevSpreadPage(1, config)).toBe(null);
		});

		it("should return page 1 when on page 2", () => {
			expect(getPrevSpreadPage(2, config)).toBe(1);
		});

		it("should return page 1 when on page 3", () => {
			expect(getPrevSpreadPage(3, config)).toBe(1);
		});

		it("should return page 2 when on page 4", () => {
			expect(getPrevSpreadPage(4, config)).toBe(2);
		});

		it("should return page 2 when on page 5", () => {
			expect(getPrevSpreadPage(5, config)).toBe(2);
		});

		it("should handle landscape pages correctly", () => {
			const configWithLandscape: SpreadConfig = {
				...config,
				pageOrientations: { 3: "landscape" },
			};
			// Page 4 shows 4-5, prev is 3 (landscape, alone)
			expect(getPrevSpreadPage(4, configWithLandscape)).toBe(3);
			// Page 3 alone (landscape), prev is 2 (alone because 3 is landscape)
			expect(getPrevSpreadPage(3, configWithLandscape)).toBe(2);
		});
	});

	// ==========================================================================
	// getPreloadPages
	// ==========================================================================

	describe("getPreloadPages", () => {
		const config: SpreadConfig = {
			totalPages: 10,
			pageOrientations: {},
			showWideAlone: true,
			startOnOdd: true,
			readingDirection: "ltr",
		};

		it("should include current spread pages", () => {
			const pages = getPreloadPages(2, config, 0);
			expect(pages).toContain(2);
			expect(pages).toContain(3);
		});

		it("should preload ahead spreads", () => {
			const pages = getPreloadPages(2, config, 1);
			// Current spread 2-3, next spread 4-5
			expect(pages).toContain(2);
			expect(pages).toContain(3);
			expect(pages).toContain(4);
			expect(pages).toContain(5);
		});

		it("should preload behind spreads", () => {
			const pages = getPreloadPages(4, config, 1);
			// Current spread 4-5, prev spread 2-3
			expect(pages).toContain(2);
			expect(pages).toContain(3);
			expect(pages).toContain(4);
			expect(pages).toContain(5);
		});

		it("should preload multiple spreads ahead and behind", () => {
			const pages = getPreloadPages(4, config, 2);
			// Prev: 1, 2-3
			// Current: 4-5
			// Next: 6-7, 8-9
			expect(pages).toContain(1);
			expect(pages).toContain(2);
			expect(pages).toContain(3);
			expect(pages).toContain(4);
			expect(pages).toContain(5);
			expect(pages).toContain(6);
			expect(pages).toContain(7);
			expect(pages).toContain(8);
			expect(pages).toContain(9);
		});

		it("should not exceed book boundaries", () => {
			const pages = getPreloadPages(1, config, 5);
			expect(pages.every((p) => p >= 1 && p <= 10)).toBe(true);
		});

		it("should return sorted array", () => {
			const pages = getPreloadPages(4, config, 2);
			const sorted = [...pages].sort((a, b) => a - b);
			expect(pages).toEqual(sorted);
		});

		it("should handle single page (page 1 with startOnOdd)", () => {
			const pages = getPreloadPages(1, config, 1);
			// Current: 1, next: 2-3
			expect(pages).toContain(1);
			expect(pages).toContain(2);
			expect(pages).toContain(3);
		});
	});

	// ==========================================================================
	// Edge cases
	// ==========================================================================

	describe("edge cases", () => {
		it("should handle single-page book", () => {
			const config: SpreadConfig = {
				totalPages: 1,
				pageOrientations: {},
				showWideAlone: true,
				startOnOdd: true,
				readingDirection: "ltr",
			};
			expect(getSpreadPages(1, config)).toEqual({
				pages: [1],
				isSinglePage: true,
			});
			expect(getNextSpreadPage(1, config)).toBe(null);
			expect(getPrevSpreadPage(1, config)).toBe(null);
		});

		it("should handle two-page book with startOnOdd", () => {
			const config: SpreadConfig = {
				totalPages: 2,
				pageOrientations: {},
				showWideAlone: true,
				startOnOdd: true,
				readingDirection: "ltr",
			};
			// Page 1 alone, page 2 alone
			expect(getSpreadPages(1, config)).toEqual({
				pages: [1],
				isSinglePage: true,
			});
			expect(getSpreadPages(2, config)).toEqual({
				pages: [2],
				isSinglePage: true,
			});
		});

		it("should handle two-page book without startOnOdd", () => {
			const config: SpreadConfig = {
				totalPages: 2,
				pageOrientations: {},
				showWideAlone: true,
				startOnOdd: false,
				readingDirection: "ltr",
			};
			// Pages 1-2 as spread
			expect(getSpreadPages(1, config)).toEqual({
				pages: [1, 2],
				isSinglePage: false,
			});
			expect(getSpreadPages(2, config)).toEqual({
				pages: [1, 2],
				isSinglePage: false,
			});
		});

		it("should handle all landscape pages", () => {
			const config: SpreadConfig = {
				totalPages: 5,
				pageOrientations: {
					1: "landscape",
					2: "landscape",
					3: "landscape",
					4: "landscape",
					5: "landscape",
				},
				showWideAlone: true,
				startOnOdd: true,
				readingDirection: "ltr",
			};
			// All pages should be shown alone
			for (let i = 1; i <= 5; i++) {
				expect(getSpreadPages(i, config)).toEqual({
					pages: [i],
					isSinglePage: true,
				});
			}
		});
	});
});
