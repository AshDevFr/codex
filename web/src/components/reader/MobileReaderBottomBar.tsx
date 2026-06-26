import {
  ActionIcon,
  Box,
  Button,
  Group,
  Modal,
  NumberInput,
  Slider,
  Text,
  Transition,
} from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import {
  IconChevronLeft,
  IconChevronRight,
  IconKeyboardShow,
  IconList,
} from "@tabler/icons-react";
import { useEffect, useState } from "react";
import {
  selectEffectiveReadingDirection,
  selectProgressPercent,
  useReaderStore,
} from "@/store/readerStore";

/**
 * Optional chapter context for EPUB reflowable books. When provided, the
 * bar renders an EPUB-specific layout (no slider; a tappable chapter pill
 * in the center that opens the TOC drawer) instead of the default
 * page-counter + slider layout used by CBZ and PDF.
 *
 * EPUB pagination is reflowable, so a 1..N page slider isn't meaningful;
 * the TOC is the natural mobile navigation surface. The chapter index is
 * computed by the parent reader from `rendition.location.start.href` matched
 * against the top-level TOC array.
 */
export interface MobileBottomBarEpubChapter {
  /** 1-based index of the current chapter in the top-level TOC. */
  currentIndex: number;
  /** Total number of top-level TOC entries. */
  total: number;
  /** Tap handler. Opens the TOC drawer. */
  onTap: () => void;
}

interface MobileReaderBottomBarProps {
  /** Whether the bar is visible (mirrors the toolbar's visibility). */
  visible: boolean;
  /**
   * Optional custom prev/next handlers. When omitted we fall back to the
   * reader store actions, matching the same default used by `ReaderToolbar`.
   * Comic / PDF readers pass their spread- or boundary-aware variants here.
   */
  onPrevPage?: () => void;
  onNextPage?: () => void;
  /**
   * When set, the bar switches to its EPUB layout: chapter pill (instead of
   * page counter) and no slider. See `MobileBottomBarEpubChapter`.
   */
  epubChapter?: MobileBottomBarEpubChapter;
}

/**
 * Bottom navigation bar shown below the `xs` breakpoint (phones).
 *
 * The desktop `ReaderToolbar` packs nine controls into a single row plus a
 * full-width slider beneath, which overflows on a 390px viewport. On phones
 * the toolbar drops the slider and most controls; this bar restores them in
 * the standard mobile-reader pattern: prev / page-count tap / next / slider.
 *
 * Tap on the page-count opens a "Go to page" modal with a numeric input —
 * faster than dragging the slider when jumping a long distance.
 */
export function MobileReaderBottomBar({
  visible,
  onPrevPage,
  onNextPage,
  epubChapter,
}: MobileReaderBottomBarProps) {
  const currentPage = useReaderStore((state) => state.currentPage);
  const totalPages = useReaderStore((state) => state.totalPages);
  const progressPercent = useReaderStore(selectProgressPercent);
  const readingDirection = useReaderStore(selectEffectiveReadingDirection);
  // Whether there's an adjacent book to step into past this boundary. When
  // there is, keep the boundary chevron enabled so it can open the chapter
  // transition overlay instead of dead-ending at the first/last page.
  const hasNextBook = useReaderStore(
    (state) => state.adjacentBooks?.next != null,
  );
  const hasPrevBook = useReaderStore(
    (state) => state.adjacentBooks?.prev != null,
  );
  const setPage = useReaderStore((state) => state.setPage);
  const storeNextPage = useReaderStore((state) => state.nextPage);
  const storePrevPage = useReaderStore((state) => state.prevPage);

  const handleNext = onNextPage ?? storeNextPage;
  const handlePrev = onPrevPage ?? storePrevPage;

  // EPUB layout uses page-style prev/next (epub.js viewports) and ignores
  // the store's currentPage entirely (reflowable books don't have one).
  const isEpub = epubChapter !== undefined;

  // Chevrons mirror the reading direction so the visual cue matches the
  // direction of progression (RTL keeps "next" on the left).
  const isRtl = readingDirection === "rtl";
  const onLeftClick = isRtl ? handleNext : handlePrev;
  const onRightClick = isRtl ? handlePrev : handleNext;
  // EPUB can't easily report "first/last viewport" without tracking it in
  // the parent. Leave chevrons enabled and rely on epub.js to no-op at the
  // boundaries.
  const leftDisabled = isEpub
    ? false
    : isRtl
      ? currentPage >= totalPages && !hasNextBook
      : currentPage <= 1 && !hasPrevBook;
  const rightDisabled = isEpub
    ? false
    : isRtl
      ? currentPage <= 1 && !hasPrevBook
      : currentPage >= totalPages && !hasNextBook;

  const [jumpOpened, jumpHandlers] = useDisclosure(false);
  const [jumpValue, setJumpValue] = useState<number>(currentPage);

  // Reset the modal input each time it opens so it always reflects the
  // current page rather than a stale value from a previous open.
  useEffect(() => {
    if (jumpOpened) {
      setJumpValue(currentPage);
    }
  }, [jumpOpened, currentPage]);

  const submitJump = () => {
    const target = Math.max(1, Math.min(totalPages, Math.round(jumpValue)));
    setPage(target);
    jumpHandlers.close();
  };

  // Phone-only: above the xs breakpoint the desktop `ReaderToolbar` already
  // shows the slider, so this bar would be duplicative. xs = 30.125em.
  const isMobile = useMediaQuery("(max-width: 30.0625em)") ?? false;

  // For CBZ/PDF, bail if there's no page data. EPUB doesn't drive the store,
  // so totalPages will be 0; but if we have chapter context, we should still
  // render the bar.
  if (!isMobile) {
    return null;
  }
  if (!isEpub && totalPages <= 0) {
    return null;
  }

  return (
    <>
      <Transition mounted={visible} transition="slide-up" duration={200}>
        {(styles) => (
          <Box
            style={{
              ...styles,
              position: "absolute",
              bottom: 0,
              left: 0,
              right: 0,
              zIndex: 100,
              background:
                "linear-gradient(to top, rgba(0,0,0,0.9) 0%, rgba(0,0,0,0.7) 70%, rgba(0,0,0,0) 100%)",
              padding: "12px 16px",
              // Respect iOS home indicator + side notches when installed
              // as a PWA in standalone mode.
              paddingBottom: "calc(12px + env(safe-area-inset-bottom, 0px))",
              paddingLeft: "calc(16px + env(safe-area-inset-left, 0px))",
              paddingRight: "calc(16px + env(safe-area-inset-right, 0px))",
              // The gradient fades to transparent at the top, but the Box
              // still grabs pointer events across its whole height. In PWA
              // mode `safe-area-inset-bottom` (~34px for the home indicator)
              // and the tall gradient gobble side taps the user intends for
              // the page underneath. Pass touches through; controls below
              // re-enable pointer events on themselves.
              pointerEvents: "none",
            }}
          >
            <Group
              gap="xs"
              wrap="nowrap"
              align="center"
              style={{ pointerEvents: "auto" }}
            >
              <ActionIcon
                variant="subtle"
                color="gray"
                size="xl"
                onClick={onLeftClick}
                disabled={leftDisabled}
                aria-label={isRtl ? "Next page" : "Previous page"}
              >
                <IconChevronLeft size={22} />
              </ActionIcon>

              <Box style={{ flex: 1, minWidth: 0 }}>
                {isEpub ? (
                  // EPUB layout: centered chapter pill, tap → TOC drawer. No
                  // slider because reflowable EPUB pages don't form a discrete
                  // 1..N sequence; the TOC is the right nav surface.
                  <Group gap="xs" justify="center" wrap="nowrap">
                    <Button
                      variant="subtle"
                      color="gray"
                      size="sm"
                      onClick={epubChapter.onTap}
                      leftSection={<IconList size={16} />}
                      aria-label="Open table of contents"
                      style={{ color: "white" }}
                    >
                      Ch {epubChapter.currentIndex} / {epubChapter.total}
                    </Button>
                  </Group>
                ) : (
                  <Group gap="xs" align="center" wrap="nowrap">
                    <Button
                      variant="subtle"
                      color="gray"
                      size="xs"
                      onClick={jumpHandlers.open}
                      leftSection={<IconKeyboardShow size={14} />}
                      aria-label="Jump to page"
                      style={{ color: "white", flexShrink: 0 }}
                    >
                      {currentPage} / {totalPages}
                    </Button>
                    <Slider
                      value={currentPage}
                      min={1}
                      max={totalPages}
                      onChange={(val) =>
                        setPage(isRtl ? totalPages + 1 - val : val)
                      }
                      onChangeEnd={() => {
                        if (document.activeElement instanceof HTMLElement) {
                          document.activeElement.blur();
                        }
                      }}
                      size="md"
                      style={{
                        flex: 1,
                        minWidth: 0,
                        transform: isRtl ? "scaleX(-1)" : "none",
                      }}
                      label={(value) => `Page ${value}`}
                      styles={{
                        track: {
                          backgroundColor: "var(--mantine-color-dark-4)",
                        },
                        bar: {
                          backgroundColor: "var(--mantine-color-blue-6)",
                        },
                        thumb: {
                          backgroundColor: "var(--mantine-color-blue-6)",
                          borderColor: "var(--mantine-color-blue-6)",
                        },
                        label: {
                          transform: isRtl ? "scaleX(-1)" : "none",
                        },
                      }}
                    />
                    <Text
                      size="xs"
                      c="dimmed"
                      style={{ width: 36, textAlign: "right", flexShrink: 0 }}
                    >
                      {progressPercent}%
                    </Text>
                  </Group>
                )}
              </Box>

              <ActionIcon
                variant="subtle"
                color="gray"
                size="xl"
                onClick={onRightClick}
                disabled={rightDisabled}
                aria-label={isRtl ? "Previous page" : "Next page"}
              >
                <IconChevronRight size={22} />
              </ActionIcon>
            </Group>
          </Box>
        )}
      </Transition>

      <Modal
        // EPUB mode never exposes the page-jump button, so this modal can
        // stay mounted; it just won't open. (Modal is conditionally rendered
        // anyway to keep the EPUB DOM lean.)
        opened={jumpOpened && !isEpub}
        onClose={jumpHandlers.close}
        title="Go to page"
        centered
        size="xs"
      >
        <NumberInput
          value={jumpValue}
          onChange={(val) =>
            setJumpValue(typeof val === "number" ? val : Number(val) || 1)
          }
          min={1}
          max={totalPages}
          autoFocus
          data-autofocus
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              submitJump();
            }
          }}
        />
        <Text size="xs" c="dimmed" mt="xs">
          Page 1–{totalPages}
        </Text>
        <Group justify="flex-end" mt="md">
          <Button variant="subtle" onClick={jumpHandlers.close}>
            Cancel
          </Button>
          <Button onClick={submitJump}>Go</Button>
        </Group>
      </Modal>
    </>
  );
}
