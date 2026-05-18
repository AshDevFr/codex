import { Portal } from "@mantine/core";
import {
  AnimatePresence,
  m,
  type PanInfo,
  useDragControls,
} from "motion/react";
import { type ReactNode, useEffect, useState } from "react";
import { SPRING_SOFT } from "@/lib/motion/easings";
import { useReducedMotion } from "@/lib/motion/useReducedMotion";
import classes from "./FilterPanel.module.css";

export interface FilterBottomSheetProps {
  opened: boolean;
  onClose: () => void;
  title: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
}

type SnapPoint = "peek" | "full";

// The sheet is `position: fixed; bottom: 0; height: 90dvh`. Translate Y
// values below shift the whole sheet down; `0` is the full-snap, the
// peek-snap leaves the bottom `35dvh` of the sheet offscreen (so 55dvh
// of the sheet is visible, exposing 45dvh of the grid below).
const PEEK_Y = "35dvh";
const FULL_Y = "0dvh";
const DISMISS_Y = "100dvh";

/**
 * Mobile bottom-sheet variant of the filter panel.
 *
 * Two snap points (peek ≈ 55dvh visible, full ≈ 90dvh visible). The
 * drag handle owns the gesture (`dragListener=false` on the sheet so
 * scrolling the chip list inside never accidentally drags the sheet).
 * On release, a strong downward fling or large drag offset either
 * downgrades full→peek or dismisses; an upward gesture promotes
 * peek→full. Reduced-motion users skip the spring (instant snap).
 *
 * The component is unconditionally rendered; it bails early when
 * `opened === false` and only paints when mounted under
 * `<AnimatePresence>` so the exit transition plays before unmount.
 */
export function FilterBottomSheet({
  opened,
  onClose,
  title,
  children,
  footer,
}: FilterBottomSheetProps) {
  const [snap, setSnap] = useState<SnapPoint>("peek");
  const dragControls = useDragControls();
  const reducedMotion = useReducedMotion();

  // Reset to peek every time the sheet re-opens so it doesn't remember
  // the user's last snap from a prior open. iOS-style behaviour: each
  // open is a fresh interaction.
  useEffect(() => {
    if (opened) setSnap("peek");
  }, [opened]);

  // Escape dismisses (matches Mantine Drawer semantics).
  useEffect(() => {
    if (!opened) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [opened, onClose]);

  const handleDragEnd = (_event: PointerEvent, info: PanInfo) => {
    const velocity = info.velocity.y;
    const offset = info.offset.y;

    // Strong downward gesture → downgrade or dismiss.
    if (velocity > 800 || offset > 220) {
      if (snap === "full") {
        setSnap("peek");
      } else {
        onClose();
      }
      return;
    }
    // Strong upward gesture → promote peek → full.
    if ((velocity < -300 || offset < -100) && snap === "peek") {
      setSnap("full");
    }
    // Otherwise: the `animate` prop springs back to the current snap.
  };

  const toggleSnap = () =>
    setSnap((current) => (current === "peek" ? "full" : "peek"));

  const yTarget = snap === "full" ? FULL_Y : PEEK_Y;
  const transition = reducedMotion ? { duration: 0 } : SPRING_SOFT;

  return (
    <AnimatePresence>
      {opened && (
        <Portal>
          <m.div
            key="filter-sheet-overlay"
            className={classes.bottomSheetOverlay}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: reducedMotion ? 0 : 0.2 }}
            onClick={onClose}
            aria-hidden
          />
          <m.div
            key="filter-sheet"
            role="dialog"
            aria-modal="true"
            aria-label="Filters"
            className={classes.bottomSheet}
            data-snap={snap}
            initial={{ y: DISMISS_Y }}
            animate={{ y: yTarget }}
            exit={{ y: DISMISS_Y }}
            transition={transition}
            drag="y"
            dragControls={dragControls}
            dragListener={false}
            dragConstraints={{ top: 0 }}
            dragElastic={{ top: 0.05, bottom: 0.4 }}
            dragMomentum={false}
            onDragEnd={handleDragEnd}
          >
            {/* Drag handle row.
                - Native `<button>` so screen readers / keyboard users
                  get the right semantics for free.
                - `Enter`/`Space` toggles peek↔full via the button's
                  default activation; explicit handler keeps the click
                  path identical.
                - `onPointerDown` hands the gesture to dragControls so
                  drag is scoped to the handle (chip taps below don't
                  trigger a drag). */}
            <button
              type="button"
              className={classes.bottomSheetHandleRow}
              aria-label="Drag to resize, swipe down to dismiss"
              data-testid="filter-bottom-sheet-handle"
              onPointerDown={(event) => dragControls.start(event)}
              onClick={toggleSnap}
              onKeyDown={(event) => {
                if (event.key === "Escape") onClose();
              }}
            >
              <span className={classes.bottomSheetHandle} aria-hidden />
            </button>
            <div className={classes.bottomSheetTitle}>{title}</div>
            <div className={classes.bottomSheetBody}>{children}</div>
            {footer}
          </m.div>
        </Portal>
      )}
    </AnimatePresence>
  );
}
