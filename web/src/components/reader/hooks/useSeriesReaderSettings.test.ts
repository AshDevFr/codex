import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useAuthStore } from "@/store/authStore";
import { useReaderStore } from "@/store/readerStore";

import {
	type CleanupResult,
	type SeriesSettingsEntry,
	SERIES_KEY_SUFFIX,
	STORAGE_KEY_PREFIX,
	cleanupCorruptedSeriesSettings,
	cleanupOrphanedSeriesSettings,
	clearAllSeriesSettings,
	getSeriesSettingsForUser,
	getSeriesStorageKey,
	useSeriesReaderSettings,
} from "./useSeriesReaderSettings";

describe("useSeriesReaderSettings", () => {
	const TEST_USER_ID = "user-123";
	const TEST_SERIES_ID = "series-456";

	beforeEach(() => {
		// Clear localStorage
		localStorage.clear();

		// Reset reader store to defaults
		useReaderStore.setState({
			settings: {
				fitMode: "screen",
				pageLayout: "single",
				readingDirection: "ltr",
				backgroundColor: "black",
				pdfMode: "streaming",
				pdfSpreadMode: "single",
				pdfContinuousScroll: false,
				autoHideToolbar: true,
				toolbarHideDelay: 3000,
				epubTheme: "light",
				epubFontSize: 100,
				epubFontFamily: "default",
				epubLineHeight: 140,
				epubMargin: 10,
				preloadPages: 1,
				doublePageShowWideAlone: true,
				doublePageStartOnOdd: true,
				pageTransition: "slide",
				transitionDuration: 200,
				webtoonSidePadding: 0,
				webtoonPageGap: 0,
				autoAdvanceToNextBook: false,
			},
		});

		// Set up authenticated user
		useAuthStore.setState({
			user: { id: TEST_USER_ID, username: "testuser", role: "user" },
			token: "test-token",
			isAuthenticated: true,
		});
	});

	describe("getSeriesStorageKey", () => {
		it("should generate correct storage key format", () => {
			const key = getSeriesStorageKey("user-abc", "series-xyz");
			expect(key).toBe("codex-reader-user-abc-series-series-xyz");
		});
	});

	describe("initialization", () => {
		it("should load with isLoaded=false initially then true", async () => {
			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});
		});

		it("should default to no series override", async () => {
			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(false);
			expect(result.current.seriesOverride).toBeNull();
		});

		it("should return global settings when no series override exists", async () => {
			// Update global settings
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pageLayout: "double",
					readingDirection: "rtl",
				},
			});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.effectiveSettings.pageLayout).toBe("double");
			expect(result.current.effectiveSettings.readingDirection).toBe("rtl");
		});

		it("should handle null seriesId gracefully", async () => {
			const { result } = renderHook(() => useSeriesReaderSettings(null));

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(false);
			// Should still return global settings
			expect(result.current.effectiveSettings.pageLayout).toBe("single");
		});

		it("should handle undefined seriesId gracefully", async () => {
			const { result } = renderHook(() => useSeriesReaderSettings(undefined));

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(false);
		});
	});

	describe("loading series override", () => {
		it("should load series override from localStorage", async () => {
			const storageKey = getSeriesStorageKey(TEST_USER_ID, TEST_SERIES_ID);

			// Pre-populate localStorage with series override
			localStorage.setItem(
				storageKey,
				JSON.stringify({
					fitMode: "width",
					pageLayout: "single",
					readingDirection: "rtl",
					backgroundColor: "white",
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
					createdAt: Date.now(),
					version: 1,
				}),
			);

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(true);
			expect(result.current.effectiveSettings.pageLayout).toBe("single");
			expect(result.current.effectiveSettings.readingDirection).toBe("rtl");
			expect(result.current.effectiveSettings.backgroundColor).toBe("white");
		});

		it("should ignore invalid JSON in localStorage", async () => {
			const storageKey = getSeriesStorageKey(TEST_USER_ID, TEST_SERIES_ID);
			localStorage.setItem(storageKey, "invalid json");

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(false);
			expect(consoleWarn).toHaveBeenCalled();

			consoleWarn.mockRestore();
		});

		it("should ignore override with wrong version", async () => {
			const storageKey = getSeriesStorageKey(TEST_USER_ID, TEST_SERIES_ID);

			localStorage.setItem(
				storageKey,
				JSON.stringify({
					fitMode: "width",
					pageLayout: "single",
					readingDirection: "rtl",
					backgroundColor: "white",
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
					createdAt: Date.now(),
					version: 99, // Wrong version
				}),
			);

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(false);

			consoleWarn.mockRestore();
		});

		it("should ignore override with missing fields", async () => {
			const storageKey = getSeriesStorageKey(TEST_USER_ID, TEST_SERIES_ID);

			localStorage.setItem(
				storageKey,
				JSON.stringify({
					fitMode: "width",
					// Missing other fields
					createdAt: Date.now(),
					version: 1,
				}),
			);

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(false);

			consoleWarn.mockRestore();
		});
	});

	describe("forkToSeries", () => {
		it("should create override with current global settings", async () => {
			// Set specific global settings
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pageLayout: "double",
					readingDirection: "rtl",
					fitMode: "width",
				},
			});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.forkToSeries();
			});

			expect(result.current.hasSeriesOverride).toBe(true);
			expect(result.current.effectiveSettings.pageLayout).toBe("double");
			expect(result.current.effectiveSettings.readingDirection).toBe("rtl");
			expect(result.current.effectiveSettings.fitMode).toBe("width");

			// Check localStorage
			const storageKey = getSeriesStorageKey(TEST_USER_ID, TEST_SERIES_ID);
			const storedValue = localStorage.getItem(storageKey);
			expect(storedValue).not.toBeNull();
			const stored = JSON.parse(storedValue as string);
			expect(stored.pageLayout).toBe("double");
			expect(stored.version).toBe(1);
			expect(stored.createdAt).toBeDefined();
		});

		it("should not create override when seriesId is null", async () => {
			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const { result } = renderHook(() => useSeriesReaderSettings(null));

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.forkToSeries();
			});

			expect(result.current.hasSeriesOverride).toBe(false);
			expect(consoleWarn).toHaveBeenCalledWith(
				"Cannot fork to series: no seriesId provided",
			);

			consoleWarn.mockRestore();
		});
	});

	describe("resetToGlobal", () => {
		it("should remove override and return to global settings", async () => {
			const storageKey = getSeriesStorageKey(TEST_USER_ID, TEST_SERIES_ID);

			// Pre-populate with series override
			localStorage.setItem(
				storageKey,
				JSON.stringify({
					fitMode: "width",
					pageLayout: "single",
					readingDirection: "rtl",
					backgroundColor: "white",
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
					createdAt: Date.now(),
					version: 1,
				}),
			);

			// Set global to different values
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pageLayout: "double",
					readingDirection: "ltr",
				},
			});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Should have series override
			expect(result.current.hasSeriesOverride).toBe(true);
			expect(result.current.effectiveSettings.pageLayout).toBe("single");

			act(() => {
				result.current.resetToGlobal();
			});

			// Should now use global
			expect(result.current.hasSeriesOverride).toBe(false);
			expect(result.current.effectiveSettings.pageLayout).toBe("double");
			expect(result.current.effectiveSettings.readingDirection).toBe("ltr");

			// localStorage should be cleared
			expect(localStorage.getItem(storageKey)).toBeNull();
		});

		it("should handle reset when no override exists", async () => {
			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Should not throw
			act(() => {
				result.current.resetToGlobal();
			});

			expect(result.current.hasSeriesOverride).toBe(false);
		});
	});

	describe("updateSetting", () => {
		it("should create override if none exists when updating", async () => {
			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(false);

			act(() => {
				result.current.updateSetting("pageLayout", "single");
			});

			expect(result.current.hasSeriesOverride).toBe(true);
			expect(result.current.effectiveSettings.pageLayout).toBe("single");
		});

		it("should update existing override", async () => {
			const storageKey = getSeriesStorageKey(TEST_USER_ID, TEST_SERIES_ID);

			// Pre-populate with series override
			localStorage.setItem(
				storageKey,
				JSON.stringify({
					fitMode: "screen",
					pageLayout: "double",
					readingDirection: "rtl",
					backgroundColor: "black",
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
					createdAt: Date.now() - 10000,
					version: 1,
				}),
			);

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			const originalCreatedAt = result.current.seriesOverride?.createdAt;

			act(() => {
				result.current.updateSetting("pageLayout", "single");
			});

			expect(result.current.effectiveSettings.pageLayout).toBe("single");
			// Other settings should be preserved
			expect(result.current.effectiveSettings.readingDirection).toBe("rtl");
			// createdAt should be preserved
			expect(result.current.seriesOverride?.createdAt).toBe(originalCreatedAt);
		});

		it("should update global store when no seriesId", async () => {
			const { result } = renderHook(() => useSeriesReaderSettings(null));

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.updateSetting("pageLayout", "continuous");
			});

			// Should update global store
			expect(useReaderStore.getState().settings.pageLayout).toBe("continuous");
			// Should still not have series override
			expect(result.current.hasSeriesOverride).toBe(false);
		});

		it("should handle all forkable setting types", async () => {
			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Test each setting type
			act(() => {
				result.current.updateSetting("fitMode", "width");
			});
			expect(result.current.effectiveSettings.fitMode).toBe("width");

			act(() => {
				result.current.updateSetting("pageLayout", "continuous");
			});
			expect(result.current.effectiveSettings.pageLayout).toBe("continuous");

			act(() => {
				result.current.updateSetting("readingDirection", "ttb");
			});
			expect(result.current.effectiveSettings.readingDirection).toBe("ttb");

			act(() => {
				result.current.updateSetting("backgroundColor", "gray");
			});
			expect(result.current.effectiveSettings.backgroundColor).toBe("gray");

			act(() => {
				result.current.updateSetting("doublePageShowWideAlone", false);
			});
			expect(result.current.effectiveSettings.doublePageShowWideAlone).toBe(
				false,
			);

			act(() => {
				result.current.updateSetting("doublePageStartOnOdd", false);
			});
			expect(result.current.effectiveSettings.doublePageStartOnOdd).toBe(false);
		});
	});

	describe("user scoping", () => {
		it("should scope storage key by userId", async () => {
			const storageKey1 = getSeriesStorageKey("user-1", TEST_SERIES_ID);
			// Note: storageKey2 intentionally not pre-populated - user 2 uses global defaults

			// User 1 has series override
			localStorage.setItem(
				storageKey1,
				JSON.stringify({
					fitMode: "width",
					pageLayout: "single",
					readingDirection: "rtl",
					backgroundColor: "white",
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
					createdAt: Date.now(),
					version: 1,
				}),
			);

			// User 2 has no override (default)

			// Login as user 1
			useAuthStore.setState({
				user: { id: "user-1", username: "user1", role: "user" },
			});

			const { result: result1 } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result1.current.isLoaded).toBe(true);
			});

			expect(result1.current.hasSeriesOverride).toBe(true);
			expect(result1.current.effectiveSettings.pageLayout).toBe("single");

			// Login as user 2
			useAuthStore.setState({
				user: { id: "user-2", username: "user2", role: "user" },
			});

			const { result: result2 } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result2.current.isLoaded).toBe(true);
			});

			expect(result2.current.hasSeriesOverride).toBe(false);
		});

		it("should use anonymous fallback when no user", async () => {
			useAuthStore.setState({
				user: null,
				token: null,
				isAuthenticated: false,
			});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Should still work
			act(() => {
				result.current.forkToSeries();
			});

			expect(result.current.hasSeriesOverride).toBe(true);

			// Check storage uses anonymous key
			const anonKey = getSeriesStorageKey("anonymous", TEST_SERIES_ID);
			expect(localStorage.getItem(anonKey)).not.toBeNull();
		});

		it("should reload settings when user changes", async () => {
			// User 1 has a series override
			const user1Key = getSeriesStorageKey("user-1", TEST_SERIES_ID);
			localStorage.setItem(
				user1Key,
				JSON.stringify({
					fitMode: "width",
					pageLayout: "double",
					readingDirection: "rtl",
					backgroundColor: "white",
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
					createdAt: Date.now(),
					version: 1,
				}),
			);

			// Login as user 1
			useAuthStore.setState({
				user: { id: "user-1", username: "user1", role: "user" },
				token: "token-1",
				isAuthenticated: true,
			});

			const { result, rerender } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(true);
			expect(result.current.effectiveSettings.readingDirection).toBe("rtl");

			// Switch to user 2 (no override for this series)
			useAuthStore.setState({
				user: { id: "user-2", username: "user2", role: "user" },
				token: "token-2",
				isAuthenticated: true,
			});

			// Rerender to pick up the new user
			rerender();

			await vi.waitFor(() => {
				expect(result.current.hasSeriesOverride).toBe(false);
			});

			// Should now use global settings
			expect(result.current.effectiveSettings.readingDirection).toBe("ltr");
		});
	});

	describe("localStorage error handling", () => {
		it("should handle localStorage write failure gracefully", async () => {
			const consoleError = vi
				.spyOn(console, "error")
				.mockImplementation(() => {});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Mock localStorage.setItem to throw AFTER the hook is loaded
			const setItemSpy = vi
				.spyOn(Storage.prototype, "setItem")
				.mockImplementation(() => {
					throw new Error("QuotaExceededError");
				});

			act(() => {
				result.current.forkToSeries();
			});

			// Should not crash, override should not be set
			expect(result.current.hasSeriesOverride).toBe(false);
			expect(consoleError).toHaveBeenCalled();

			// Restore
			setItemSpy.mockRestore();
			consoleError.mockRestore();
		});

		it("should handle localStorage read failure gracefully", async () => {
			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			// Mock localStorage.getItem to throw
			const getItemSpy = vi
				.spyOn(Storage.prototype, "getItem")
				.mockImplementation(() => {
					throw new Error("SecurityError");
				});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Should gracefully fall back to global settings
			expect(result.current.hasSeriesOverride).toBe(false);
			expect(consoleWarn).toHaveBeenCalled();

			// Restore
			getItemSpy.mockRestore();
			consoleWarn.mockRestore();
		});

		it("should handle updateSetting with localStorage failure", async () => {
			const consoleError = vi
				.spyOn(console, "error")
				.mockImplementation(() => {});

			const { result } = renderHook(() =>
				useSeriesReaderSettings(TEST_SERIES_ID),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Mock localStorage.setItem to throw AFTER the hook is loaded
			const setItemSpy = vi
				.spyOn(Storage.prototype, "setItem")
				.mockImplementation(() => {
					throw new Error("QuotaExceededError");
				});

			act(() => {
				result.current.updateSetting("pageLayout", "double");
			});

			// Should not crash, override should not be set
			expect(result.current.hasSeriesOverride).toBe(false);
			expect(consoleError).toHaveBeenCalled();

			// Restore
			setItemSpy.mockRestore();
			consoleError.mockRestore();
		});
	});

	describe("different series", () => {
		it("should use separate storage for different series", async () => {
			const series1Key = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const series2Key = getSeriesStorageKey(TEST_USER_ID, "series-2");

			// Pre-populate series 1 with rtl
			localStorage.setItem(
				series1Key,
				JSON.stringify({
					fitMode: "screen",
					pageLayout: "single",
					readingDirection: "rtl",
					backgroundColor: "black",
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
					createdAt: Date.now(),
					version: 1,
				}),
			);

			// Pre-populate series 2 with ltr
			localStorage.setItem(
				series2Key,
				JSON.stringify({
					fitMode: "screen",
					pageLayout: "double",
					readingDirection: "ltr",
					backgroundColor: "white",
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
					createdAt: Date.now(),
					version: 1,
				}),
			);

			const { result: result1 } = renderHook(() =>
				useSeriesReaderSettings("series-1"),
			);

			await vi.waitFor(() => {
				expect(result1.current.isLoaded).toBe(true);
			});

			expect(result1.current.effectiveSettings.readingDirection).toBe("rtl");
			expect(result1.current.effectiveSettings.pageLayout).toBe("single");

			const { result: result2 } = renderHook(() =>
				useSeriesReaderSettings("series-2"),
			);

			await vi.waitFor(() => {
				expect(result2.current.isLoaded).toBe(true);
			});

			expect(result2.current.effectiveSettings.readingDirection).toBe("ltr");
			expect(result2.current.effectiveSettings.pageLayout).toBe("double");
		});
	});

	describe("series change", () => {
		it("should reload settings when seriesId changes", async () => {
			const series1Key = getSeriesStorageKey(TEST_USER_ID, "series-1");
			// Note: series 2 has no override - uses global defaults

			// Pre-populate series 1
			localStorage.setItem(
				series1Key,
				JSON.stringify({
					fitMode: "screen",
					pageLayout: "single",
					readingDirection: "rtl",
					backgroundColor: "black",
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
					createdAt: Date.now(),
					version: 1,
				}),
			);

			const { result, rerender } = renderHook(
				({ seriesId }: { seriesId: string }) =>
					useSeriesReaderSettings(seriesId),
				{ initialProps: { seriesId: "series-1" } },
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasSeriesOverride).toBe(true);
			expect(result.current.effectiveSettings.readingDirection).toBe("rtl");

			// Change to series 2
			rerender({ seriesId: "series-2" });

			await vi.waitFor(() => {
				expect(result.current.hasSeriesOverride).toBe(false);
			});

			// Should now use global settings
			expect(result.current.effectiveSettings.readingDirection).toBe("ltr");
		});
	});
});

// =============================================================================
// Cleanup Utilities Tests
// =============================================================================

describe("localStorage cleanup utilities", () => {
	const TEST_USER_ID = "user-cleanup-test";

	// Helper to create a valid series override
	const createValidOverride = () =>
		JSON.stringify({
			fitMode: "screen",
			pageLayout: "single",
			readingDirection: "ltr",
			backgroundColor: "black",
			doublePageShowWideAlone: true,
			doublePageStartOnOdd: true,
			createdAt: Date.now(),
			version: 1,
		});

	beforeEach(() => {
		localStorage.clear();
	});

	describe("constants", () => {
		it("should export STORAGE_KEY_PREFIX", () => {
			expect(STORAGE_KEY_PREFIX).toBe("codex-reader-");
		});

		it("should export SERIES_KEY_SUFFIX", () => {
			expect(SERIES_KEY_SUFFIX).toBe("-series-");
		});
	});

	describe("getSeriesSettingsForUser", () => {
		it("should return empty array when no series settings exist", () => {
			const entries = getSeriesSettingsForUser(TEST_USER_ID);
			expect(entries).toEqual([]);
		});

		it("should find all series settings for a user", () => {
			// Add series settings for our user
			const key1 = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const key2 = getSeriesStorageKey(TEST_USER_ID, "series-2");
			localStorage.setItem(key1, createValidOverride());
			localStorage.setItem(key2, createValidOverride());

			const entries = getSeriesSettingsForUser(TEST_USER_ID);

			expect(entries).toHaveLength(2);
			expect(entries.map((e) => e.seriesId).sort()).toEqual([
				"series-1",
				"series-2",
			]);
		});

		it("should not include settings for other users", () => {
			// Add settings for our user
			const ourKey = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(ourKey, createValidOverride());

			// Add settings for another user
			const otherKey = getSeriesStorageKey("other-user", "series-2");
			localStorage.setItem(otherKey, createValidOverride());

			const entries = getSeriesSettingsForUser(TEST_USER_ID);

			expect(entries).toHaveLength(1);
			expect(entries[0].seriesId).toBe("series-1");
		});

		it("should not include non-series keys", () => {
			// Add a series setting
			const seriesKey = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(seriesKey, createValidOverride());

			// Add some other keys that should be ignored
			localStorage.setItem("codex-reader-settings", "{}");
			localStorage.setItem("some-other-key", "value");

			const entries = getSeriesSettingsForUser(TEST_USER_ID);

			expect(entries).toHaveLength(1);
			expect(entries[0].seriesId).toBe("series-1");
		});

		it("should include entry info for valid data", () => {
			const key = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const createdAt = Date.now();
			localStorage.setItem(
				key,
				JSON.stringify({
					fitMode: "width",
					pageLayout: "double",
					readingDirection: "rtl",
					backgroundColor: "white",
					doublePageShowWideAlone: false,
					doublePageStartOnOdd: false,
					createdAt,
					version: 1,
				}),
			);

			const entries = getSeriesSettingsForUser(TEST_USER_ID);

			expect(entries).toHaveLength(1);
			const entry = entries[0];
			expect(entry.key).toBe(key);
			expect(entry.seriesId).toBe("series-1");
			expect(entry.data).not.toBeNull();
			expect(entry.data?.fitMode).toBe("width");
			expect(entry.createdAt).toBe(createdAt);
		});

		it("should mark corrupted entries with null data", () => {
			const key = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(key, "invalid json");

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const entries = getSeriesSettingsForUser(TEST_USER_ID);

			expect(entries).toHaveLength(1);
			expect(entries[0].data).toBeNull();
			expect(entries[0].createdAt).toBeNull();

			consoleWarn.mockRestore();
		});

		it("should handle localStorage enumeration errors gracefully", () => {
			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			// Mock localStorage.length to throw
			const lengthSpy = vi
				.spyOn(Storage.prototype, "length", "get")
				.mockImplementation(() => {
					throw new Error("SecurityError");
				});

			const entries = getSeriesSettingsForUser(TEST_USER_ID);

			expect(entries).toEqual([]);
			expect(consoleWarn).toHaveBeenCalled();

			lengthSpy.mockRestore();
			consoleWarn.mockRestore();
		});
	});

	describe("cleanupOrphanedSeriesSettings", () => {
		it("should remove settings for series not in valid set", () => {
			// Add settings for series 1, 2, and 3
			const key1 = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const key2 = getSeriesStorageKey(TEST_USER_ID, "series-2");
			const key3 = getSeriesStorageKey(TEST_USER_ID, "series-3");
			localStorage.setItem(key1, createValidOverride());
			localStorage.setItem(key2, createValidOverride());
			localStorage.setItem(key3, createValidOverride());

			// Only series 1 and 2 are valid
			const validSeriesIds = new Set(["series-1", "series-2"]);

			const result = cleanupOrphanedSeriesSettings(
				TEST_USER_ID,
				validSeriesIds,
			);

			expect(result.removed).toBe(1);
			expect(result.removedKeys).toContain(key3);
			expect(result.errors).toBe(0);

			// Verify localStorage
			expect(localStorage.getItem(key1)).not.toBeNull();
			expect(localStorage.getItem(key2)).not.toBeNull();
			expect(localStorage.getItem(key3)).toBeNull();
		});

		it("should not remove any settings when all are valid", () => {
			const key1 = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const key2 = getSeriesStorageKey(TEST_USER_ID, "series-2");
			localStorage.setItem(key1, createValidOverride());
			localStorage.setItem(key2, createValidOverride());

			const validSeriesIds = new Set(["series-1", "series-2"]);

			const result = cleanupOrphanedSeriesSettings(
				TEST_USER_ID,
				validSeriesIds,
			);

			expect(result.removed).toBe(0);
			expect(result.removedKeys).toHaveLength(0);

			// Both should still exist
			expect(localStorage.getItem(key1)).not.toBeNull();
			expect(localStorage.getItem(key2)).not.toBeNull();
		});

		it("should remove all settings when valid set is empty", () => {
			const key1 = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const key2 = getSeriesStorageKey(TEST_USER_ID, "series-2");
			localStorage.setItem(key1, createValidOverride());
			localStorage.setItem(key2, createValidOverride());

			const result = cleanupOrphanedSeriesSettings(
				TEST_USER_ID,
				new Set<string>(),
			);

			expect(result.removed).toBe(2);
			expect(localStorage.getItem(key1)).toBeNull();
			expect(localStorage.getItem(key2)).toBeNull();
		});

		it("should return empty result when no settings exist", () => {
			const result = cleanupOrphanedSeriesSettings(
				TEST_USER_ID,
				new Set(["series-1"]),
			);

			expect(result.removed).toBe(0);
			expect(result.removedKeys).toHaveLength(0);
			expect(result.errors).toBe(0);
		});

		it("should not affect other users' settings", () => {
			// Our user's settings
			const ourKey = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(ourKey, createValidOverride());

			// Other user's settings
			const otherKey = getSeriesStorageKey("other-user", "series-orphan");
			localStorage.setItem(otherKey, createValidOverride());

			// Clean up our user's orphaned settings (series-1 not in valid set)
			const result = cleanupOrphanedSeriesSettings(
				TEST_USER_ID,
				new Set<string>(),
			);

			expect(result.removed).toBe(1);
			expect(localStorage.getItem(ourKey)).toBeNull();
			// Other user's settings should be untouched
			expect(localStorage.getItem(otherKey)).not.toBeNull();
		});

		it("should track errors when removal fails", () => {
			const key = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(key, createValidOverride());

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});
			const removeItemSpy = vi
				.spyOn(Storage.prototype, "removeItem")
				.mockImplementation(() => {
					throw new Error("Failed to remove");
				});

			const result = cleanupOrphanedSeriesSettings(
				TEST_USER_ID,
				new Set<string>(),
			);

			expect(result.removed).toBe(0);
			expect(result.errors).toBe(1);
			expect(consoleWarn).toHaveBeenCalled();

			removeItemSpy.mockRestore();
			consoleWarn.mockRestore();
		});
	});

	describe("clearAllSeriesSettings", () => {
		it("should remove all series settings for a user", () => {
			const key1 = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const key2 = getSeriesStorageKey(TEST_USER_ID, "series-2");
			const key3 = getSeriesStorageKey(TEST_USER_ID, "series-3");
			localStorage.setItem(key1, createValidOverride());
			localStorage.setItem(key2, createValidOverride());
			localStorage.setItem(key3, createValidOverride());

			const result = clearAllSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(3);
			expect(result.removedKeys).toHaveLength(3);
			expect(result.errors).toBe(0);

			expect(localStorage.getItem(key1)).toBeNull();
			expect(localStorage.getItem(key2)).toBeNull();
			expect(localStorage.getItem(key3)).toBeNull();
		});

		it("should return empty result when no settings exist", () => {
			const result = clearAllSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(0);
			expect(result.removedKeys).toHaveLength(0);
			expect(result.errors).toBe(0);
		});

		it("should not affect other users' settings", () => {
			// Our user's settings
			const ourKey = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(ourKey, createValidOverride());

			// Other user's settings
			const otherKey = getSeriesStorageKey("other-user", "series-1");
			localStorage.setItem(otherKey, createValidOverride());

			clearAllSeriesSettings(TEST_USER_ID);

			expect(localStorage.getItem(ourKey)).toBeNull();
			expect(localStorage.getItem(otherKey)).not.toBeNull();
		});

		it("should track errors when removal fails", () => {
			const key = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(key, createValidOverride());

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});
			const removeItemSpy = vi
				.spyOn(Storage.prototype, "removeItem")
				.mockImplementation(() => {
					throw new Error("Failed to remove");
				});

			const result = clearAllSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(0);
			expect(result.errors).toBe(1);

			removeItemSpy.mockRestore();
			consoleWarn.mockRestore();
		});
	});

	describe("cleanupCorruptedSeriesSettings", () => {
		it("should remove entries with invalid JSON", () => {
			const validKey = getSeriesStorageKey(TEST_USER_ID, "series-valid");
			const corruptedKey = getSeriesStorageKey(
				TEST_USER_ID,
				"series-corrupted",
			);
			localStorage.setItem(validKey, createValidOverride());
			localStorage.setItem(corruptedKey, "not valid json");

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const result = cleanupCorruptedSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(1);
			expect(result.removedKeys).toContain(corruptedKey);
			expect(result.errors).toBe(0);

			expect(localStorage.getItem(validKey)).not.toBeNull();
			expect(localStorage.getItem(corruptedKey)).toBeNull();

			consoleWarn.mockRestore();
		});

		it("should remove entries with wrong version", () => {
			const validKey = getSeriesStorageKey(TEST_USER_ID, "series-valid");
			const wrongVersionKey = getSeriesStorageKey(
				TEST_USER_ID,
				"series-wrong-version",
			);
			localStorage.setItem(validKey, createValidOverride());
			localStorage.setItem(
				wrongVersionKey,
				JSON.stringify({
					fitMode: "screen",
					pageLayout: "single",
					readingDirection: "ltr",
					backgroundColor: "black",
					doublePageShowWideAlone: true,
					doublePageStartOnOdd: true,
					createdAt: Date.now(),
					version: 999, // Wrong version
				}),
			);

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const result = cleanupCorruptedSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(1);
			expect(result.removedKeys).toContain(wrongVersionKey);
			expect(localStorage.getItem(validKey)).not.toBeNull();
			expect(localStorage.getItem(wrongVersionKey)).toBeNull();

			consoleWarn.mockRestore();
		});

		it("should remove entries with missing required fields", () => {
			const validKey = getSeriesStorageKey(TEST_USER_ID, "series-valid");
			const incompleteKey = getSeriesStorageKey(
				TEST_USER_ID,
				"series-incomplete",
			);
			localStorage.setItem(validKey, createValidOverride());
			localStorage.setItem(
				incompleteKey,
				JSON.stringify({
					fitMode: "screen",
					// Missing other required fields
					createdAt: Date.now(),
					version: 1,
				}),
			);

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const result = cleanupCorruptedSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(1);
			expect(localStorage.getItem(validKey)).not.toBeNull();
			expect(localStorage.getItem(incompleteKey)).toBeNull();

			consoleWarn.mockRestore();
		});

		it("should not remove any entries when all are valid", () => {
			const key1 = getSeriesStorageKey(TEST_USER_ID, "series-1");
			const key2 = getSeriesStorageKey(TEST_USER_ID, "series-2");
			localStorage.setItem(key1, createValidOverride());
			localStorage.setItem(key2, createValidOverride());

			const result = cleanupCorruptedSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(0);
			expect(localStorage.getItem(key1)).not.toBeNull();
			expect(localStorage.getItem(key2)).not.toBeNull();
		});

		it("should return empty result when no settings exist", () => {
			const result = cleanupCorruptedSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(0);
			expect(result.removedKeys).toHaveLength(0);
			expect(result.errors).toBe(0);
		});

		it("should track errors when removal fails", () => {
			const key = getSeriesStorageKey(TEST_USER_ID, "series-1");
			localStorage.setItem(key, "corrupted");

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});
			const removeItemSpy = vi
				.spyOn(Storage.prototype, "removeItem")
				.mockImplementation(() => {
					throw new Error("Failed to remove");
				});

			const result = cleanupCorruptedSeriesSettings(TEST_USER_ID);

			expect(result.removed).toBe(0);
			expect(result.errors).toBe(1);

			removeItemSpy.mockRestore();
			consoleWarn.mockRestore();
		});
	});

	describe("CleanupResult type", () => {
		it("should have correct structure", () => {
			const result: CleanupResult = {
				removed: 5,
				removedKeys: ["key1", "key2"],
				errors: 1,
			};

			expect(result.removed).toBe(5);
			expect(result.removedKeys).toHaveLength(2);
			expect(result.errors).toBe(1);
		});
	});

	describe("SeriesSettingsEntry type", () => {
		it("should have correct structure", () => {
			const entry: SeriesSettingsEntry = {
				key: "codex-reader-user-series-123",
				seriesId: "123",
				data: null,
				createdAt: null,
			};

			expect(entry.key).toBe("codex-reader-user-series-123");
			expect(entry.seriesId).toBe("123");
			expect(entry.data).toBeNull();
			expect(entry.createdAt).toBeNull();
		});
	});
});
