import {
	ActionIcon,
	Box,
	Group,
	Slider,
	Text,
	Tooltip,
	Transition,
} from "@mantine/core";
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
}

const FIT_MODE_LABELS: Record<FitMode, string> = {
	screen: "Fit to Screen",
	width: "Fit Width",
	"width-shrink": "Fit Width (Shrink Only)",
	height: "Fit Height",
	original: "Original Size",
};

/**
 * Toolbar component for the reader.
 *
 * Shows:
 * - Book title
 * - Page navigation controls
 * - Progress slider
 * - Fit mode indicator
 * - Fullscreen toggle
 * - Settings button
 */
export function ReaderToolbar({
	title,
	visible,
	onClose,
	onOpenSettings,
	showPageNavigation = true,
	leftActions,
	rightActions,
	prevBook,
	nextBook,
	onPrevBook,
	onNextBook,
	fitMode: fitModeProp,
	onCycleFitMode,
	pageLayout,
	onTogglePageLayout,
	hasSeriesOverride = false,
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

	// Adjust navigation based on reading direction
	// In RTL mode, clicking the left chevron goes forward (next page)
	// In LTR mode, clicking the left chevron goes backward (previous page)
	// Icons stay visually consistent - only behavior and tooltips change
	const onLeftClick = readingDirection === "ltr" ? prevPage : nextPage;
	const onRightClick = readingDirection === "ltr" ? nextPage : prevPage;
	const leftTooltip =
		readingDirection === "ltr" ? "Previous page" : "Next page";
	const rightTooltip =
		readingDirection === "ltr" ? "Next page" : "Previous page";
	const leftDisabled =
		readingDirection === "ltr" ? currentPage <= 1 : currentPage >= totalPages;
	const rightDisabled =
		readingDirection === "ltr" ? currentPage >= totalPages : currentPage <= 1;

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
							"linear-gradient(to bottom, rgba(0,0,0,0.8) 0%, rgba(0,0,0,0) 100%)",
						padding: "12px 16px",
					}}
				>
					{/* Top row: Title, controls, close */}
					<Group justify="space-between" mb="xs">
						{/* Left: Close button, title, and custom actions */}
						<Group gap="xs">
							<Tooltip label="Close reader (Esc)">
								<ActionIcon
									variant="subtle"
									color="gray"
									onClick={onClose}
									size="lg"
								>
									<IconX size={20} />
								</ActionIcon>
							</Tooltip>
							<Text size="sm" fw={500} c="white" lineClamp={1} maw={300}>
								{title}
							</Text>
							{leftActions}
						</Group>

						{/* Center: Navigation controls */}
						{showPageNavigation && (
							<Group gap="xs">
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
											size="lg"
										>
											<IconPlayerSkipBack size={18} />
										</ActionIcon>
									</Tooltip>
								)}

								<Tooltip label={leftTooltip}>
									<ActionIcon
										variant="subtle"
										color="gray"
										onClick={onLeftClick}
										disabled={leftDisabled}
										size="lg"
									>
										<IconChevronLeft size={20} />
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
										size="lg"
									>
										<IconChevronRight size={20} />
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
											size="lg"
										>
											<IconPlayerSkipForward size={18} />
										</ActionIcon>
									</Tooltip>
								)}
							</Group>
						)}

						{/* Right: Actions */}
						<Group gap="xs">
							{showPageNavigation && (
								<Tooltip label={`Fit mode: ${FIT_MODE_LABELS[fitMode]} (M)`}>
									<ActionIcon
										variant="subtle"
										color={hasSeriesOverride ? "blue" : "gray"}
										onClick={cycleFitMode}
										size="lg"
									>
										{fitMode === "screen" && <IconAspectRatio size={20} />}
										{fitMode === "width" && <IconArrowAutofitWidth size={20} />}
										{fitMode === "width-shrink" && (
											<IconArrowAutofitDown size={20} />
										)}
										{fitMode === "height" && (
											<IconArrowAutofitHeight size={20} />
										)}
										{fitMode === "original" && <IconPhoto size={20} />}
									</ActionIcon>
								</Tooltip>
							)}

							{/* Page layout toggle - only show for paginated modes */}
							{showPageNavigation &&
								onTogglePageLayout &&
								pageLayout &&
								pageLayout !== "continuous" && (
									<Tooltip
										label={`Page layout: ${pageLayout === "single" ? "Single" : "Double"}`}
									>
										<ActionIcon
											variant="subtle"
											color={hasSeriesOverride ? "blue" : "gray"}
											onClick={onTogglePageLayout}
											size="lg"
										>
											{pageLayout === "single" ? (
												<IconFile size={20} />
											) : (
												<IconBook size={20} />
											)}
										</ActionIcon>
									</Tooltip>
								)}

							{rightActions}

							<Tooltip
								label={isFullscreen ? "Exit fullscreen (F)" : "Fullscreen (F)"}
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									onClick={toggleFullscreen}
									size="lg"
								>
									{isFullscreen ? (
										<IconArrowsMinimize size={20} />
									) : (
										<IconArrowsMaximize size={20} />
									)}
								</ActionIcon>
							</Tooltip>

							{onOpenSettings && (
								<Tooltip label="Settings">
									<ActionIcon
										variant="subtle"
										color="gray"
										onClick={onOpenSettings}
										size="lg"
									>
										<IconSettings size={20} />
									</ActionIcon>
								</Tooltip>
							)}
						</Group>
					</Group>

					{/* Bottom row: Progress slider (only for page-based readers) */}
					{showPageNavigation && (
						<Box px="md">
							<Group gap="xs" align="center">
								<Text
									size="xs"
									c="dimmed"
									w={40}
									style={{ textAlign: "right" }}
								>
									{progressPercent}%
								</Text>
								<Slider
									value={currentPage}
									min={1}
									max={totalPages}
									onChange={setPage}
									size="xs"
									style={{ flex: 1 }}
									label={(value) => `Page ${value}`}
									styles={{
										bar: { backgroundColor: "var(--mantine-color-blue-6)" },
										thumb: {
											backgroundColor: "var(--mantine-color-blue-6)",
											borderColor: "var(--mantine-color-blue-6)",
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
