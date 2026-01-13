import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSearchShortcut } from "./useSearchShortcut";

describe("useSearchShortcut", () => {
	let mockFocus: ReturnType<typeof vi.fn>;
	let searchInputRef: { current: { focus: () => void } | null };

	beforeEach(() => {
		mockFocus = vi.fn();
		searchInputRef = { current: { focus: mockFocus } };
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	const dispatchKeyDown = (
		key: string,
		options: Partial<KeyboardEventInit> = {},
	) => {
		const event = new KeyboardEvent("keydown", {
			key,
			bubbles: true,
			...options,
		});
		window.dispatchEvent(event);
	};

	it("should focus search input when 's' is pressed", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		dispatchKeyDown("s");

		expect(mockFocus).toHaveBeenCalledTimes(1);
	});

	it("should focus search input when 'S' is pressed", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		dispatchKeyDown("S");

		expect(mockFocus).toHaveBeenCalledTimes(1);
	});

	it("should not focus when ctrl+s is pressed", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		dispatchKeyDown("s", { ctrlKey: true });

		expect(mockFocus).not.toHaveBeenCalled();
	});

	it("should not focus when alt+s is pressed", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		dispatchKeyDown("s", { altKey: true });

		expect(mockFocus).not.toHaveBeenCalled();
	});

	it("should not focus when meta+s is pressed", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		dispatchKeyDown("s", { metaKey: true });

		expect(mockFocus).not.toHaveBeenCalled();
	});

	it("should not focus when shift+s is pressed", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		dispatchKeyDown("s", { shiftKey: true });

		expect(mockFocus).not.toHaveBeenCalled();
	});

	it("should not focus when a different key is pressed", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		dispatchKeyDown("a");
		dispatchKeyDown("Enter");
		dispatchKeyDown("Escape");

		expect(mockFocus).not.toHaveBeenCalled();
	});

	it("should not focus when focus is on an input element", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		const input = document.createElement("input");
		document.body.appendChild(input);
		input.focus();

		const event = new KeyboardEvent("keydown", {
			key: "s",
			bubbles: true,
		});
		Object.defineProperty(event, "target", { value: input });
		window.dispatchEvent(event);

		expect(mockFocus).not.toHaveBeenCalled();

		document.body.removeChild(input);
	});

	it("should not focus when focus is on a textarea element", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		const textarea = document.createElement("textarea");
		document.body.appendChild(textarea);
		textarea.focus();

		const event = new KeyboardEvent("keydown", {
			key: "s",
			bubbles: true,
		});
		Object.defineProperty(event, "target", { value: textarea });
		window.dispatchEvent(event);

		expect(mockFocus).not.toHaveBeenCalled();

		document.body.removeChild(textarea);
	});

	it("should not focus when focus is on a contentEditable element", () => {
		renderHook(() => useSearchShortcut({ searchInputRef }));

		const div = document.createElement("div");
		div.setAttribute("contenteditable", "true");
		document.body.appendChild(div);
		div.focus();

		// Create a mock element with isContentEditable set to true
		const mockTarget = {
			tagName: "DIV",
			isContentEditable: true
		};

		const event = new KeyboardEvent("keydown", {
			key: "s",
			bubbles: true,
		});
		Object.defineProperty(event, "target", { value: mockTarget });
		window.dispatchEvent(event);

		expect(mockFocus).not.toHaveBeenCalled();

		document.body.removeChild(div);
	});

	it("should not add listener when disabled", () => {
		renderHook(() => useSearchShortcut({ searchInputRef, enabled: false }));

		dispatchKeyDown("s");

		expect(mockFocus).not.toHaveBeenCalled();
	});

	it("should remove listener on unmount", () => {
		const { unmount } = renderHook(() =>
			useSearchShortcut({ searchInputRef }),
		);

		unmount();

		dispatchKeyDown("s");

		expect(mockFocus).not.toHaveBeenCalled();
	});

	it("should handle null ref gracefully", () => {
		const nullRef = { current: null };

		renderHook(() => useSearchShortcut({ searchInputRef: nullRef }));

		// Should not throw
		expect(() => dispatchKeyDown("s")).not.toThrow();
	});
});
