import { useCallback, useEffect } from "react";
import type { RefObject } from "react";
import type { SearchInputHandle } from "@/components/search";

interface UseSearchShortcutOptions {
	/** Ref to the search input component */
	searchInputRef: RefObject<SearchInputHandle | null>;
	/** Whether the shortcut is enabled (default: true) */
	enabled?: boolean;
}

/**
 * Hook for global 'S' keyboard shortcut to focus the search bar.
 *
 * The shortcut is ignored when:
 * - Focus is on an input, textarea, or contentEditable element
 * - Any modifier key (Ctrl, Alt, Meta, Shift) is pressed
 */
export function useSearchShortcut({
	searchInputRef,
	enabled = true,
}: UseSearchShortcutOptions) {
	const handleKeyDown = useCallback(
		(event: KeyboardEvent) => {
			// Only trigger on 'S' or 's' without any modifiers
			if (event.key !== "s" && event.key !== "S") {
				return;
			}

			// Don't trigger if any modifier is pressed
			if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) {
				return;
			}

			// Don't trigger if focus is on an input element
			const target = event.target as HTMLElement;
			if (
				target.tagName === "INPUT" ||
				target.tagName === "TEXTAREA" ||
				target.isContentEditable
			) {
				return;
			}

			event.preventDefault();
			searchInputRef.current?.focus();
		},
		[searchInputRef],
	);

	useEffect(() => {
		if (!enabled) return;

		window.addEventListener("keydown", handleKeyDown);
		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, [enabled, handleKeyDown]);
}
