import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useReaderStore } from "@/store/readerStore";

import { usePerBookSettings } from "./usePerBookSettings";

describe("usePerBookSettings", () => {
	const STORAGE_KEY_PREFIX = "codex-book-settings-";

	beforeEach(() => {
		// Clear localStorage
		localStorage.clear();

		// Reset reader store
		useReaderStore.setState({
			settings: {
				...useReaderStore.getState().settings,
				pdfMode: "streaming",
			},
		});
	});

	describe("initialization", () => {
		it("should load with isLoaded=false initially then true", async () => {
			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			// After effects run, should be loaded
			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});
		});

		it("should default to no per-book preference", async () => {
			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasPerBookPdfMode).toBe(false);
		});

		it("should use global PDF mode when no per-book preference", async () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pdfMode: "native",
				},
			});

			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.effectivePdfMode).toBe("native");
		});
	});

	describe("loading per-book settings", () => {
		it("should load per-book PDF mode from localStorage", async () => {
			// Pre-populate localStorage
			localStorage.setItem(
				`${STORAGE_KEY_PREFIX}book-123`,
				JSON.stringify({ pdfMode: "native" }),
			);

			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasPerBookPdfMode).toBe(true);
			expect(result.current.effectivePdfMode).toBe("native");
		});

		it("should not apply PDF mode for non-PDF books", async () => {
			// Pre-populate localStorage
			localStorage.setItem(
				`${STORAGE_KEY_PREFIX}book-123`,
				JSON.stringify({ pdfMode: "native" }),
			);

			// Global mode is streaming
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pdfMode: "streaming",
				},
			});

			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "CBZ"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Should not have changed the global mode
			expect(result.current.hasPerBookPdfMode).toBe(false);
			expect(result.current.effectivePdfMode).toBe("streaming");
		});

		it("should handle invalid JSON in localStorage gracefully", async () => {
			// Put invalid JSON in localStorage
			localStorage.setItem(`${STORAGE_KEY_PREFIX}book-123`, "invalid json");

			const consoleWarn = vi
				.spyOn(console, "warn")
				.mockImplementation(() => {});

			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			expect(result.current.hasPerBookPdfMode).toBe(false);
			expect(consoleWarn).toHaveBeenCalled();

			consoleWarn.mockRestore();
		});
	});

	describe("savePerBookPdfMode", () => {
		it("should save PDF mode to localStorage", async () => {
			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.savePerBookPdfMode("native");
			});

			expect(result.current.hasPerBookPdfMode).toBe(true);
			expect(result.current.effectivePdfMode).toBe("native");

			// Check localStorage
			const stored = localStorage.getItem(`${STORAGE_KEY_PREFIX}book-123`);
			expect(JSON.parse(stored!)).toEqual({ pdfMode: "native" });
		});

		it("should update store with new PDF mode", async () => {
			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.savePerBookPdfMode("native");
			});

			expect(useReaderStore.getState().settings.pdfMode).toBe("native");
		});

		it("should preserve other settings when saving", async () => {
			// Pre-populate with other settings
			localStorage.setItem(
				`${STORAGE_KEY_PREFIX}book-123`,
				JSON.stringify({ someOtherSetting: "value" }),
			);

			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.savePerBookPdfMode("native");
			});

			const stored = localStorage.getItem(`${STORAGE_KEY_PREFIX}book-123`);
			expect(JSON.parse(stored!)).toEqual({
				someOtherSetting: "value",
				pdfMode: "native",
			});
		});
	});

	describe("clearPerBookPdfMode", () => {
		it("should remove PDF mode from localStorage", async () => {
			// Pre-populate
			localStorage.setItem(
				`${STORAGE_KEY_PREFIX}book-123`,
				JSON.stringify({ pdfMode: "native" }),
			);

			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.clearPerBookPdfMode();
			});

			expect(result.current.hasPerBookPdfMode).toBe(false);

			// localStorage should be removed (was only pdfMode)
			expect(localStorage.getItem(`${STORAGE_KEY_PREFIX}book-123`)).toBeNull();
		});

		it("should preserve other settings when clearing", async () => {
			// Pre-populate with multiple settings
			localStorage.setItem(
				`${STORAGE_KEY_PREFIX}book-123`,
				JSON.stringify({ pdfMode: "native", otherSetting: "value" }),
			);

			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			act(() => {
				result.current.clearPerBookPdfMode();
			});

			const stored = localStorage.getItem(`${STORAGE_KEY_PREFIX}book-123`);
			expect(JSON.parse(stored!)).toEqual({ otherSetting: "value" });
		});

		it("should handle clearing when no settings exist", async () => {
			const { result } = renderHook(() =>
				usePerBookSettings("book-123", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result.current.isLoaded).toBe(true);
			});

			// Should not throw
			act(() => {
				result.current.clearPerBookPdfMode();
			});

			expect(result.current.hasPerBookPdfMode).toBe(false);
		});
	});

	describe("different books", () => {
		it("should use separate storage for different books", async () => {
			// Save for book 1
			localStorage.setItem(
				`${STORAGE_KEY_PREFIX}book-1`,
				JSON.stringify({ pdfMode: "native" }),
			);

			// Save for book 2
			localStorage.setItem(
				`${STORAGE_KEY_PREFIX}book-2`,
				JSON.stringify({ pdfMode: "streaming" }),
			);

			const { result: result1 } = renderHook(() =>
				usePerBookSettings("book-1", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result1.current.isLoaded).toBe(true);
			});

			expect(result1.current.effectivePdfMode).toBe("native");

			const { result: result2 } = renderHook(() =>
				usePerBookSettings("book-2", "PDF"),
			);

			await vi.waitFor(() => {
				expect(result2.current.isLoaded).toBe(true);
			});

			expect(result2.current.effectivePdfMode).toBe("streaming");
		});
	});
});
