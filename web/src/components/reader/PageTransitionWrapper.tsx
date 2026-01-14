import { Box } from "@mantine/core";
import { useEffect, useRef, useState } from "react";
import type {
	NavigationDirection,
	PageTransition,
	ReadingDirection,
} from "@/store/readerStore";

interface PageTransitionWrapperProps {
	/** Current page key (used to detect page changes) */
	pageKey: string;
	/** Children to render (the page content) */
	children: React.ReactNode;
	/** Type of transition animation */
	transition: PageTransition;
	/** Duration of transition in ms */
	duration: number;
	/** Direction of last navigation (for slide direction) */
	navigationDirection: NavigationDirection;
	/** Reading direction (affects slide direction) */
	readingDirection: ReadingDirection;
}

type TransitionPhase = "idle" | "entering" | "active";

interface TransitionState {
	/** Currently displayed content */
	currentContent: React.ReactNode;
	/** Previous content (during transition) */
	previousContent: React.ReactNode | null;
	/** Current page key */
	currentKey: string;
	/** Transition phase: idle (no transition), entering (start), active (animating) */
	phase: TransitionPhase;
	/** Direction for this transition */
	slideDirection: "left" | "right" | "up" | "down";
}

/**
 * Calculate slide direction based on navigation and reading direction.
 *
 * For LTR: next = slide in from right, prev = slide in from left
 * For RTL: next = slide in from left, prev = slide in from right (reversed)
 * For TTB: next = slide in from bottom (down), prev = slide in from top (up)
 */
export function getSlideDirection(
	navigationDirection: NavigationDirection,
	readingDirection: ReadingDirection,
): "left" | "right" | "up" | "down" {
	if (navigationDirection === null) {
		return readingDirection === "ttb" ? "down" : "right";
	}

	const isNext = navigationDirection === "next";

	if (readingDirection === "ttb") {
		// Vertical transitions for TTB
		return isNext ? "down" : "up";
	}

	if (readingDirection === "rtl") {
		// Reversed horizontal for RTL
		return isNext ? "left" : "right";
	}

	// Standard horizontal for LTR
	return isNext ? "right" : "left";
}

/**
 * Wrapper component that handles animated page transitions.
 *
 * Supports three transition types:
 * - none: Instant page change (no animation)
 * - fade: Crossfade between pages
 * - slide: Pages slide in from the direction of navigation
 *
 * For slide transitions:
 * - LTR + next: New page slides in from right
 * - LTR + prev: New page slides in from left
 * - RTL + next: New page slides in from left (reversed)
 * - RTL + prev: New page slides in from right (reversed)
 * - TTB + next: New page slides in from bottom (vertical)
 * - TTB + prev: New page slides in from top (vertical)
 */
export function PageTransitionWrapper({
	pageKey,
	children,
	transition,
	duration,
	navigationDirection,
	readingDirection,
}: PageTransitionWrapperProps) {
	const [state, setState] = useState<TransitionState>({
		currentContent: children,
		previousContent: null,
		currentKey: pageKey,
		phase: "idle",
		slideDirection: "right",
	});

	const transitionTimeoutRef = useRef<NodeJS.Timeout | null>(null);
	const rafRef = useRef<number | null>(null);
	const previousKeyRef = useRef<string>(pageKey);

	// Cleanup timeouts on unmount
	useEffect(() => {
		return () => {
			if (transitionTimeoutRef.current) {
				clearTimeout(transitionTimeoutRef.current);
			}
			if (rafRef.current) {
				cancelAnimationFrame(rafRef.current);
			}
		};
	}, []);

	// Handle page changes
	useEffect(() => {
		// If the page key hasn't changed, only update content when idle
		// to avoid disrupting ongoing transitions
		if (pageKey === previousKeyRef.current) {
			setState((prev) => {
				if (prev.phase === "idle") {
					return {
						...prev,
						currentContent: children,
					};
				}
				return prev;
			});
			return;
		}

		// Page changed - start transition
		previousKeyRef.current = pageKey;

		// Clear any pending transitions
		if (transitionTimeoutRef.current) {
			clearTimeout(transitionTimeoutRef.current);
			transitionTimeoutRef.current = null;
		}
		if (rafRef.current) {
			cancelAnimationFrame(rafRef.current);
			rafRef.current = null;
		}

		if (transition === "none") {
			// No animation - instant change
			setState({
				currentContent: children,
				previousContent: null,
				currentKey: pageKey,
				phase: "idle",
				slideDirection: "right",
			});
			return;
		}

		const slideDirection = getSlideDirection(
			navigationDirection,
			readingDirection,
		);

		// Start transition: set entering phase (positions elements)
		setState((prev) => ({
			currentContent: children,
			previousContent: prev.currentContent,
			currentKey: pageKey,
			phase: "entering",
			slideDirection,
		}));

		// Trigger animation after browser paints initial position
		// Use double rAF to ensure layout is complete before animating
		rafRef.current = requestAnimationFrame(() => {
			rafRef.current = requestAnimationFrame(() => {
				setState((prev) => ({
					...prev,
					phase: "active",
				}));
				rafRef.current = null;
			});
		});

		// End transition after duration (add buffer for rAF delays ~32ms for double rAF)
		transitionTimeoutRef.current = setTimeout(() => {
			setState((prev) => ({
				...prev,
				previousContent: null,
				phase: "idle",
			}));
			transitionTimeoutRef.current = null;
		}, duration + 50);
	}, [pageKey, children, transition, duration, navigationDirection, readingDirection]);

	// When idle or no transition, render content in a stable container
	// Use same structure as during transitions to prevent layout shifts
	if (transition === "none" || state.phase === "idle") {
		return (
			<Box
				style={{
					position: "relative",
					width: "100%",
					height: "100%",
					overflow: "hidden",
				}}
			>
				<Box
					style={{
						position: "absolute",
						inset: 0,
					}}
				>
					{state.currentContent}
				</Box>
			</Box>
		);
	}

	const isEntering = state.phase === "entering";
	const isActive = state.phase === "active";
	const { slideDirection } = state;

	// Calculate transforms for slide transition
	const getEnterTransform = () => {
		if (transition === "fade") return undefined;
		switch (slideDirection) {
			case "right":
				return "translateX(100%)";
			case "left":
				return "translateX(-100%)";
			case "down":
				return "translateY(100%)";
			case "up":
				return "translateY(-100%)";
		}
	};

	const getExitTransform = () => {
		if (transition === "fade") return undefined;
		switch (slideDirection) {
			case "right":
				return "translateX(-100%)";
			case "left":
				return "translateX(100%)";
			case "down":
				return "translateY(-100%)";
			case "up":
				return "translateY(100%)";
		}
	};

	const getNoTransform = () => {
		// Return the appropriate neutral transform based on direction
		return slideDirection === "up" || slideDirection === "down"
			? "translateY(0)"
			: "translateX(0)";
	};

	return (
		<Box
			style={{
				position: "relative",
				width: "100%",
				height: "100%",
				overflow: "hidden",
			}}
		>
			{/* Previous content (exits) */}
			{state.previousContent && (
				<Box
					style={{
						position: "absolute",
						inset: 0,
						willChange: "transform, opacity",
						backfaceVisibility: "hidden",
						transition: isActive
							? `transform ${duration}ms ease-out, opacity ${duration}ms ease-out`
							: undefined,
						opacity: transition === "fade" && isActive ? 0 : 1,
						transform:
							transition === "slide" && isActive
								? getExitTransform()
								: getNoTransform(),
					}}
				>
					{state.previousContent}
				</Box>
			)}

			{/* Current content (enters) */}
			<Box
				style={{
					position: "absolute",
					inset: 0,
					willChange: "transform, opacity",
					backfaceVisibility: "hidden",
					transition: isActive
						? `transform ${duration}ms ease-out, opacity ${duration}ms ease-out`
						: undefined,
					opacity: transition === "fade" ? (isEntering ? 0 : 1) : 1,
					transform:
						transition === "slide"
							? isEntering
								? getEnterTransform()
								: getNoTransform()
							: undefined,
				}}
			>
				{state.currentContent}
			</Box>
		</Box>
	);
}
