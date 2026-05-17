import { Box } from "@mantine/core";
import { useEffect, useRef, useState } from "react";
import type {
  NavigationDirection,
  PageTransition,
  ReadingDirection,
} from "@/store/readerStore";

/**
 * Wait for an image element to be decoded and ready to paint.
 * Falls back to load/error events on browsers without decode() support.
 */
function whenImageReady(img: HTMLImageElement): Promise<void> {
  if (typeof img.decode === "function") {
    return img.decode().catch(() => undefined);
  }
  if (img.complete) return Promise.resolve();
  return new Promise<void>((resolve) => {
    img.addEventListener("load", () => resolve(), { once: true });
    img.addEventListener("error", () => resolve(), { once: true });
  });
}

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
  const decodeTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const rafRef = useRef<number | null>(null);
  const previousKeyRef = useRef<string>(pageKey);
  const isInitialMountRef = useRef<boolean>(true);
  const currentBoxRef = useRef<HTMLDivElement>(null);
  // Cancellation token for the in-flight transition. Set to {cancelled: true}
  // when a new page change arrives so any pending image-decode promise short
  // circuits and we don't fire setState into a stale transition.
  const pendingTransitionRef = useRef<{ cancelled: boolean } | null>(null);

  // Cleanup timeouts on unmount
  useEffect(() => {
    return () => {
      if (transitionTimeoutRef.current) {
        clearTimeout(transitionTimeoutRef.current);
      }
      if (decodeTimeoutRef.current) {
        clearTimeout(decodeTimeoutRef.current);
      }
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current);
      }
      if (pendingTransitionRef.current) {
        pendingTransitionRef.current.cancelled = true;
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

    // Cancel any in-flight transition
    if (transitionTimeoutRef.current) {
      clearTimeout(transitionTimeoutRef.current);
      transitionTimeoutRef.current = null;
    }
    if (decodeTimeoutRef.current) {
      clearTimeout(decodeTimeoutRef.current);
      decodeTimeoutRef.current = null;
    }
    if (rafRef.current) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
    if (pendingTransitionRef.current) {
      pendingTransitionRef.current.cancelled = true;
    }

    // Skip transition on initial mount (when first loading the book or reloading the page)
    // Also skip when navigationDirection is null - this means no user navigation has happened yet
    // (e.g., loading saved progress, URL navigation, or programmatic page changes during init)
    if (isInitialMountRef.current || navigationDirection === null) {
      isInitialMountRef.current = false;
      setState({
        currentContent: children,
        previousContent: null,
        currentKey: pageKey,
        phase: "idle",
        slideDirection: "right",
      });
      return;
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

    // Commit the entering phase. The next useEffect below watches for
    // phase === "entering" and schedules the active phase once the new
    // page's images are decoded. Splitting into two effects guarantees
    // React fully commits and paints the off-screen starting position
    // before we begin the slide; doing both in one effect risks React
    // batching the state updates so the browser never sees the starting
    // position (the cause of "Sometimes slide is not applied").
    setState((prev) => ({
      currentContent: children,
      previousContent: prev.currentContent,
      currentKey: pageKey,
      phase: "entering",
      slideDirection,
    }));
  }, [pageKey, children, transition, navigationDirection, readingDirection]);

  // Drive the entering -> active -> idle phase progression. This runs
  // after React commits the entering state, so by the time we query the
  // DOM for <img> elements they are already mounted with the new src
  // and the browser has had a paint cycle to begin decoding.
  useEffect(() => {
    if (state.phase !== "entering") return;

    const token = { cancelled: false };
    pendingTransitionRef.current = token;

    const startActivePhase = () => {
      if (token.cancelled) return;
      token.cancelled = true;
      if (decodeTimeoutRef.current) {
        clearTimeout(decodeTimeoutRef.current);
        decodeTimeoutRef.current = null;
      }
      setState((prev) => ({ ...prev, phase: "active" }));

      // End transition after duration (add buffer for paint cycles)
      transitionTimeoutRef.current = setTimeout(() => {
        setState((prev) => ({
          ...prev,
          previousContent: null,
          phase: "idle",
        }));
        transitionTimeoutRef.current = null;
      }, duration + 50);
    };

    // Wait for the new page's images to be decoded before starting the
    // slide. Without this, an image that's cached-but-not-yet-painted
    // shows as the page's background color (typically black) for the
    // first 1-2 frames of the slide, producing the "black flicker on
    // the side it's sliding from".
    const imgs = currentBoxRef.current
      ? Array.from(currentBoxRef.current.querySelectorAll("img"))
      : [];

    if (imgs.length === 0) {
      // No images to wait for; just give the browser a frame to paint
      // the entering position before we start the transition.
      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = null;
        startActivePhase();
      });
    } else {
      // Cap the wait so a slow/broken image doesn't stall the UI.
      decodeTimeoutRef.current = setTimeout(startActivePhase, 250);

      Promise.all(imgs.map(whenImageReady)).then(() => {
        if (token.cancelled) return;
        // One rAF after decode so the decoded pixels make it to the
        // screen before the transform transition begins.
        rafRef.current = requestAnimationFrame(() => {
          rafRef.current = null;
          startActivePhase();
        });
      });
    }

    return () => {
      token.cancelled = true;
    };
  }, [state.phase, duration]);

  const isEntering = state.phase === "entering";
  const isActive = state.phase === "active";
  const isTransitioning = state.phase !== "idle" && transition !== "none";
  const { slideDirection } = state;

  // Calculate transforms for slide transition
  const getEnterTransform = () => {
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

  // Render the SAME DOM structure regardless of phase so the current
  // content's wrapper Box (and the page component inside it) is never
  // remounted across the entering/active/idle transitions. Remounting
  // would re-trigger the image's load-time opacity fade and produce a
  // flicker at the end of the slide.
  //
  // For fade: keep the previous layer fully opaque underneath and only
  // fade the new layer in over it. A true crossfade (both layers at 0.5
  // opacity at the midpoint) darkens because the bottom is composited
  // over the transparent container, yielding ~0.25 contribution from
  // the previous page and a visibly dark midpoint when pages have dark
  // backgrounds.
  return (
    <Box
      style={{
        position: "relative",
        width: "100%",
        height: "100%",
        overflow: "hidden",
      }}
    >
      {/* Previous content - only rendered during a transition. Stays
          fully opaque under the new layer for fade; slides out for slide. */}
      {state.previousContent && transition !== "none" && (
        <Box
          style={{
            position: "absolute",
            inset: 0,
            zIndex: 1,
            willChange: transition === "slide" ? "transform" : undefined,
            backfaceVisibility: "hidden",
            transition:
              isActive && transition === "slide"
                ? `transform ${duration}ms ease-out`
                : undefined,
            transform:
              transition === "slide" && isActive
                ? getExitTransform()
                : undefined,
          }}
        >
          {state.previousContent}
        </Box>
      )}

      {/* Current content - ALWAYS rendered in the same DOM position so
          React preserves the underlying page component across phases. */}
      <Box
        ref={currentBoxRef}
        style={{
          position: "absolute",
          inset: 0,
          zIndex: 2,
          willChange: isTransitioning ? "transform, opacity" : undefined,
          backfaceVisibility: "hidden",
          transition: isActive
            ? `transform ${duration}ms ease-out, opacity ${duration}ms ease-out`
            : undefined,
          opacity: transition === "fade" && isEntering ? 0 : 1,
          transform:
            transition === "slide" && isEntering
              ? getEnterTransform()
              : undefined,
        }}
      >
        {state.currentContent}
      </Box>
    </Box>
  );
}
