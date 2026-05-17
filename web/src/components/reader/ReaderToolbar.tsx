import {
  ActionIcon,
  Box,
  Group,
  Menu,
  Slider,
  Text,
  Tooltip,
  Transition,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import {
  IconArrowAutofitDown,
  IconArrowAutofitHeight,
  IconArrowAutofitWidth,
  IconArrowsMaximize,
  IconArrowsMinimize,
  IconAspectRatio,
  IconBook,
  IconChevronLeft,
  IconChevronRight,
  IconDotsVertical,
  IconFile,
  IconPhoto,
  IconPlayerSkipBack,
  IconPlayerSkipForward,
  IconSettings,
  IconX,
} from "@tabler/icons-react";
import {
  type FitMode,
  type PageLayout,
  selectEffectiveReadingDirection,
  selectProgressPercent,
  useReaderStore,
} from "@/store/readerStore";

interface ReaderToolbarProps {
  /** Book title */
  title: string;
  /** Whether the toolbar is visible */
  visible: boolean;
  /** Callback when back/close button is clicked */
  onClose: () => void;
  /** Callback when settings button is clicked */
  onOpenSettings?: () => void;
  /** Whether to show page navigation controls (default: true) */
  showPageNavigation?: boolean;
  /** Additional actions to render in the left section (after title) */
  leftActions?: React.ReactNode;
  /** Additional actions to render in the right section (before settings) */
  rightActions?: React.ReactNode;
  /**
   * Additional menu items to render in the mobile overflow menu.
   * Used to surface format-specific actions (e.g. TOC / bookmarks / search
   * for EPUB) that don't fit in the phone-sized top bar.
   */
  mobileMenuItems?: React.ReactNode;
  /** Series navigation: previous book info */
  prevBook?: { title: string } | null;
  /** Series navigation: next book info */
  nextBook?: { title: string } | null;
  /** Callback when previous book button is clicked */
  onPrevBook?: () => void;
  /** Callback when next book button is clicked */
  onNextBook?: () => void;
  /** Current fit mode (uses global store if not provided) */
  fitMode?: FitMode;
  /** Callback when fit mode button is clicked (uses global cycleFitMode if not provided) */
  onCycleFitMode?: () => void;
  /** Current page layout */
  pageLayout?: PageLayout;
  /** Callback when page layout button is clicked */
  onTogglePageLayout?: () => void;
  /** Whether series-specific settings are active (shows blue tint on buttons) */
  hasSeriesOverride?: boolean;
  /** Whether the reader is in continuous scroll mode (hides layout toggle) */
  isContinuousScroll?: boolean;
}

const FIT_MODE_LABELS: Record<FitMode, string> = {
  screen: "Fit to Screen",
  width: "Fit Width",
  "width-shrink": "Fit Width (Shrink Only)",
  height: "Fit Height",
  original: "Original Size",
};

function getFitModeIcon(fitMode: FitMode, size: number) {
  switch (fitMode) {
    case "screen":
      return <IconAspectRatio size={size} />;
    case "width":
      return <IconArrowAutofitWidth size={size} />;
    case "width-shrink":
      return <IconArrowAutofitDown size={size} />;
    case "height":
      return <IconArrowAutofitHeight size={size} />;
    case "original":
      return <IconPhoto size={size} />;
  }
}

/**
 * Toolbar component for the reader.
 *
 * Above the `xs` breakpoint: shows title, page nav, slider, fit-mode,
 * page-layout, fullscreen, and settings inline.
 *
 * Below `xs` (phones): drops the inline slider row and collapses secondary
 * actions (prev/next book, fit mode, page layout, fullscreen) into a single
 * overflow `Menu`. Page navigation and the slider move to
 * `MobileReaderBottomBar`, which is rendered separately by the parent reader.
 */
export function ReaderToolbar({
  title,
  visible,
  onClose,
  onOpenSettings,
  showPageNavigation = true,
  leftActions,
  rightActions,
  mobileMenuItems,
  prevBook,
  nextBook,
  onPrevBook,
  onNextBook,
  fitMode: fitModeProp,
  onCycleFitMode,
  pageLayout,
  onTogglePageLayout,
  hasSeriesOverride = false,
  isContinuousScroll = false,
}: ReaderToolbarProps) {
  const currentPage = useReaderStore((state) => state.currentPage);
  const totalPages = useReaderStore((state) => state.totalPages);
  const isFullscreen = useReaderStore((state) => state.isFullscreen);
  const globalFitMode = useReaderStore((state) => state.settings.fitMode);
  const progressPercent = useReaderStore(selectProgressPercent);
  const readingDirection = useReaderStore(selectEffectiveReadingDirection);

  const setPage = useReaderStore((state) => state.setPage);
  const nextPage = useReaderStore((state) => state.nextPage);
  const prevPage = useReaderStore((state) => state.prevPage);
  const toggleFullscreen = useReaderStore((state) => state.toggleFullscreen);
  const globalCycleFitMode = useReaderStore((state) => state.cycleFitMode);

  // Use prop values if provided, otherwise fall back to global store
  const fitMode = fitModeProp ?? globalFitMode;
  const cycleFitMode = onCycleFitMode ?? globalCycleFitMode;

  // Phone-only: drop the slider row from the top bar and collapse
  // secondary actions into an overflow menu. xs breakpoint = 30.125em.
  const isMobile = useMediaQuery("(max-width: 30.0625em)") ?? false;

  // Adjust navigation based on reading direction.
  // Only RTL reverses the chevrons; LTR, TTB, and webtoon all use
  // left=previous, right=next (matching the natural page order).
  const isRtl = readingDirection === "rtl";
  const onLeftClick = isRtl ? nextPage : prevPage;
  const onRightClick = isRtl ? prevPage : nextPage;
  const leftTooltip = isRtl ? "Next page" : "Previous page";
  const rightTooltip = isRtl ? "Previous page" : "Next page";
  const leftDisabled = isRtl ? currentPage >= totalPages : currentPage <= 1;
  const rightDisabled = isRtl ? currentPage <= 1 : currentPage >= totalPages;

  const actionIconSize = isMobile ? "xl" : "lg";
  const iconSize = isMobile ? 22 : 20;
  const overrideColor = hasSeriesOverride ? "blue" : "gray";
  const showLayoutToggle =
    showPageNavigation &&
    !!onTogglePageLayout &&
    !!pageLayout &&
    pageLayout !== "continuous" &&
    !isContinuousScroll;

  return (
    <Transition mounted={visible} transition="slide-down" duration={200}>
      {(styles) => (
        <Box
          style={{
            ...styles,
            position: "absolute",
            top: 0,
            left: 0,
            right: 0,
            zIndex: 100,
            background:
              "linear-gradient(to bottom, rgba(0,0,0,0.9) 0%, rgba(0,0,0,0.7) 70%, rgba(0,0,0,0) 100%)",
            padding: "12px 16px",
            // Respect iOS notch / status bar when installed as PWA in
            // standalone mode. Falls back to 0 on browsers without the var.
            paddingTop: "calc(12px + env(safe-area-inset-top, 0px))",
            paddingLeft: "calc(16px + env(safe-area-inset-left, 0px))",
            paddingRight: "calc(16px + env(safe-area-inset-right, 0px))",
            // The gradient fades to transparent at the bottom, but the Box
            // still captures pointer events across its full height. In PWA
            // standalone mode `safe-area-inset-top` (~47px) makes that area
            // tall enough to swallow taps that the user intends for the
            // page underneath. Pass pointer events through and re-enable
            // them on the actual controls below.
            pointerEvents: "none",
          }}
        >
          {/* Top row: Title, controls, close.
              Re-enable pointer events here so the controls remain tappable
              while the surrounding gradient area passes touches through. */}
          <Group
            justify="space-between"
            mb={isMobile ? 0 : "xs"}
            wrap="nowrap"
            style={{ pointerEvents: "auto" }}
          >
            {/* Left: Close button, title, and custom actions */}
            <Group gap="xs" wrap="nowrap" style={{ minWidth: 0, flex: 1 }}>
              <Tooltip label="Close reader (Esc)">
                <ActionIcon
                  variant="subtle"
                  color="gray"
                  onClick={onClose}
                  size={actionIconSize}
                  aria-label="Close reader"
                >
                  <IconX size={iconSize} />
                </ActionIcon>
              </Tooltip>
              <Text
                size="sm"
                fw={500}
                c="white"
                lineClamp={1}
                style={{ minWidth: 0, flex: 1 }}
                maw={isMobile ? undefined : 300}
              >
                {title}
              </Text>
              {/* leftActions stays mounted so portaled drawer bodies (EPUB
                  TOC/bookmarks) can still respond to parent-controlled opened
                  state on mobile. Only the trigger UI is visually hidden. */}
              {leftActions && (
                <Box
                  style={{
                    display: isMobile ? "none" : "contents",
                  }}
                >
                  {leftActions}
                </Box>
              )}
            </Group>

            {/* Center: Navigation controls (desktop only — mobile gets a bottom bar) */}
            {!isMobile && showPageNavigation && (
              <Group gap="xs" wrap="nowrap">
                {/* Previous book button */}
                {onPrevBook && (
                  <Tooltip
                    label={
                      prevBook
                        ? `Previous: ${prevBook.title}`
                        : "No previous book"
                    }
                  >
                    <ActionIcon
                      variant="subtle"
                      color="gray"
                      onClick={onPrevBook}
                      disabled={!prevBook}
                      size={actionIconSize}
                      aria-label="Previous book"
                    >
                      <IconPlayerSkipBack size={iconSize - 2} />
                    </ActionIcon>
                  </Tooltip>
                )}

                <Tooltip label={leftTooltip}>
                  <ActionIcon
                    variant="subtle"
                    color="gray"
                    onClick={onLeftClick}
                    disabled={leftDisabled}
                    size={actionIconSize}
                    aria-label={leftTooltip}
                  >
                    <IconChevronLeft size={iconSize} />
                  </ActionIcon>
                </Tooltip>

                <Text
                  size="sm"
                  c="white"
                  style={{ minWidth: 80, textAlign: "center" }}
                >
                  {currentPage} / {totalPages}
                </Text>

                <Tooltip label={rightTooltip}>
                  <ActionIcon
                    variant="subtle"
                    color="gray"
                    onClick={onRightClick}
                    disabled={rightDisabled}
                    size={actionIconSize}
                    aria-label={rightTooltip}
                  >
                    <IconChevronRight size={iconSize} />
                  </ActionIcon>
                </Tooltip>

                {/* Next book button */}
                {onNextBook && (
                  <Tooltip
                    label={
                      nextBook ? `Next: ${nextBook.title}` : "No next book"
                    }
                  >
                    <ActionIcon
                      variant="subtle"
                      color="gray"
                      onClick={onNextBook}
                      disabled={!nextBook}
                      size={actionIconSize}
                      aria-label="Next book"
                    >
                      <IconPlayerSkipForward size={iconSize - 2} />
                    </ActionIcon>
                  </Tooltip>
                )}
              </Group>
            )}

            {/* Right: Actions.
                rightActions stays mounted in both layouts so portaled drawer
                bodies (e.g. EPUB bookmarks/search) keep responding to
                parent-controlled `opened` state when their trigger UI is
                hidden on mobile. */}
            <Group gap="xs" wrap="nowrap">
              {rightActions && (
                <Box
                  style={{
                    display: isMobile ? "none" : "contents",
                  }}
                >
                  {rightActions}
                </Box>
              )}
              {isMobile ? (
                /* Mobile: collapse secondary actions into an overflow menu.
                   Settings stays as its own button because it's the highest-
                   traffic non-navigation action. */
                <>
                  {onOpenSettings && (
                    <Tooltip label="Settings">
                      <ActionIcon
                        variant="subtle"
                        color="gray"
                        onClick={onOpenSettings}
                        size={actionIconSize}
                        aria-label="Reader settings"
                      >
                        <IconSettings size={iconSize} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                  <Menu
                    shadow="md"
                    position="bottom-end"
                    withinPortal
                    keepMounted={false}
                  >
                    <Menu.Target>
                      <ActionIcon
                        variant="subtle"
                        color="gray"
                        size={actionIconSize}
                        aria-label="More reader options"
                      >
                        <IconDotsVertical size={iconSize} />
                      </ActionIcon>
                    </Menu.Target>
                    <Menu.Dropdown>
                      {showPageNavigation && (
                        <Menu.Item
                          leftSection={getFitModeIcon(fitMode, 18)}
                          onClick={cycleFitMode}
                        >
                          Fit: {FIT_MODE_LABELS[fitMode]}
                        </Menu.Item>
                      )}
                      {showLayoutToggle && (
                        <Menu.Item
                          leftSection={
                            pageLayout === "single" ? (
                              <IconFile size={18} />
                            ) : (
                              <IconBook size={18} />
                            )
                          }
                          onClick={onTogglePageLayout}
                        >
                          Layout:{" "}
                          {pageLayout === "single" ? "Single" : "Double"}
                        </Menu.Item>
                      )}
                      <Menu.Item
                        leftSection={
                          isFullscreen ? (
                            <IconArrowsMinimize size={18} />
                          ) : (
                            <IconArrowsMaximize size={18} />
                          )
                        }
                        onClick={toggleFullscreen}
                      >
                        {isFullscreen ? "Exit fullscreen" : "Fullscreen"}
                      </Menu.Item>
                      {onPrevBook && (
                        <Menu.Item
                          leftSection={<IconPlayerSkipBack size={18} />}
                          onClick={onPrevBook}
                          disabled={!prevBook}
                        >
                          {prevBook
                            ? `Previous: ${prevBook.title}`
                            : "No previous book"}
                        </Menu.Item>
                      )}
                      {onNextBook && (
                        <Menu.Item
                          leftSection={<IconPlayerSkipForward size={18} />}
                          onClick={onNextBook}
                          disabled={!nextBook}
                        >
                          {nextBook
                            ? `Next: ${nextBook.title}`
                            : "No next book"}
                        </Menu.Item>
                      )}
                      {mobileMenuItems}
                    </Menu.Dropdown>
                  </Menu>
                </>
              ) : (
                <>
                  {showPageNavigation && (
                    <Tooltip
                      label={`Fit mode: ${FIT_MODE_LABELS[fitMode]} (M)`}
                    >
                      <ActionIcon
                        variant="subtle"
                        color={overrideColor}
                        onClick={cycleFitMode}
                        size={actionIconSize}
                        aria-label="Cycle fit mode"
                      >
                        {getFitModeIcon(fitMode, iconSize)}
                      </ActionIcon>
                    </Tooltip>
                  )}

                  {/* Page layout toggle - only show for paginated modes */}
                  {showLayoutToggle && (
                    <Tooltip
                      label={`Page layout: ${pageLayout === "single" ? "Single" : "Double"}`}
                    >
                      <ActionIcon
                        variant="subtle"
                        color={overrideColor}
                        onClick={onTogglePageLayout}
                        size={actionIconSize}
                        aria-label="Toggle page layout"
                      >
                        {pageLayout === "single" ? (
                          <IconFile size={iconSize} />
                        ) : (
                          <IconBook size={iconSize} />
                        )}
                      </ActionIcon>
                    </Tooltip>
                  )}

                  <Tooltip
                    label={
                      isFullscreen ? "Exit fullscreen (F)" : "Fullscreen (F)"
                    }
                  >
                    <ActionIcon
                      variant="subtle"
                      color="gray"
                      onClick={toggleFullscreen}
                      size={actionIconSize}
                      aria-label="Toggle fullscreen"
                    >
                      {isFullscreen ? (
                        <IconArrowsMinimize size={iconSize} />
                      ) : (
                        <IconArrowsMaximize size={iconSize} />
                      )}
                    </ActionIcon>
                  </Tooltip>

                  {onOpenSettings && (
                    <Tooltip label="Settings">
                      <ActionIcon
                        variant="subtle"
                        color="gray"
                        onClick={onOpenSettings}
                        size={actionIconSize}
                        aria-label="Reader settings"
                      >
                        <IconSettings size={iconSize} />
                      </ActionIcon>
                    </Tooltip>
                  )}
                </>
              )}
            </Group>
          </Group>

          {/* Bottom row: Progress slider (desktop only — phones use
              MobileReaderBottomBar so the top bar stays compact). The Box
              re-enables pointer events so slider/label clicks register. */}
          {!isMobile && showPageNavigation && (
            <Box px="md" style={{ pointerEvents: "auto" }}>
              <Group
                gap="xs"
                align="center"
                style={{
                  flexDirection:
                    readingDirection === "rtl" ? "row-reverse" : "row",
                }}
              >
                <Text
                  size="xs"
                  c="dimmed"
                  w={40}
                  style={{
                    textAlign: readingDirection === "rtl" ? "left" : "right",
                  }}
                >
                  {progressPercent}%
                </Text>
                <Slider
                  value={currentPage}
                  min={1}
                  max={totalPages}
                  onChange={(val) =>
                    setPage(
                      readingDirection === "rtl" ? totalPages + 1 - val : val,
                    )
                  }
                  // Blur the slider after click/drag so its built-in
                  // keyboard handler doesn't conflict with useKeyboardNav.
                  // In RTL the slider's arrow-key handler applies the
                  // inverted onChange, fighting the global handler.
                  onChangeEnd={() => {
                    if (document.activeElement instanceof HTMLElement) {
                      document.activeElement.blur();
                    }
                  }}
                  size="xs"
                  style={{
                    flex: 1,
                    transform:
                      readingDirection === "rtl" ? "scaleX(-1)" : "none",
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
                      // Counter-flip the label so text isn't mirrored
                      transform:
                        readingDirection === "rtl" ? "scaleX(-1)" : "none",
                    },
                  }}
                />
              </Group>
            </Box>
          )}
        </Box>
      )}
    </Transition>
  );
}
